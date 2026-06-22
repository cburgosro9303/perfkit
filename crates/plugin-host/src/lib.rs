//! perfkit — host de plugins WASM con permisos declarativos y firma obligatoria.
//!
//! Fase 8: plugins WASM seguros y firmados (deny-by-default).
//!
//! Un plugin de perfkit es un módulo WASM que exporta:
//! - `perfkit_abi_version() -> i32` (debe ser igual a [`ABI_VERSION`]),
//! - `run(i64) -> i64` (un cómputo puro usado como transformación de valores,
//!   p.ej. un timer/think-time custom o un checksum).
//!
//! El host **no** provee imports a menos que el manifiesto lo permita
//! (deny-by-default). Para el MVP los plugins son puros (no se otorga ningún
//! import).
//!
//! ## Seguridad
//! Un plugin **sin firmar** o **manipulado** NO debe cargar (DoD §9 Fase 8).
//! [`PluginHost::load_signed`] exige que:
//! 1. `manifest.sha256` coincida con el SHA-256 de los bytes wasm,
//! 2. la firma ed25519 valide contra al menos una clave de confianza,
//! 3. el módulo instancie correctamente,
//! 4. `perfkit_abi_version()` sea igual a [`ABI_VERSION`].
//!
//! La ejecución de [`LoadedPlugin::call_run`] está acotada por *fuel*, de modo
//! que un bucle infinito devuelve `Err(OutOfFuel)` en lugar de colgarse.

use std::collections::HashSet;

use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasmi::core::TrapCode;
use wasmi::{Config, Engine, Instance, Linker, Module, Store};

/// Versión del ABI de plugins soportada por este host.
pub const ABI_VERSION: i32 = 1;

/// Límite de *fuel* por invocación de `run`. Acota el cómputo del plugin para
/// que un bucle infinito termine en `Err(OutOfFuel)` y no cuelgue al host.
pub const DEFAULT_FUEL: u64 = 10_000_000;

/// Codificador/decodificador base64 estándar (con padding).
const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

// ---------------------------------------------------------------------------
// Permisos y manifiesto
// ---------------------------------------------------------------------------

/// Permisos declarativos de un plugin. `Default` = todo denegado (deny-by-default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Permissions {
    /// Permite acceso de red (no implementado en el MVP; siempre debe ir `false`).
    #[serde(default)]
    pub allow_net: bool,
    /// Permite leer variables de entorno (no implementado en el MVP).
    #[serde(default)]
    pub allow_env: bool,
    /// Permite acceso al sistema de archivos (no implementado en el MVP).
    #[serde(default)]
    pub allow_fs: bool,
}

impl Permissions {
    /// `true` si el plugin no solicita ningún permiso (plugin puro).
    pub fn is_pure(&self) -> bool {
        !self.allow_net && !self.allow_env && !self.allow_fs
    }
}

/// Manifiesto de un plugin: metadatos, permisos solicitados y material de firma.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Nombre legible del plugin.
    pub name: String,
    /// Versión del plugin (semver, comparada de forma exacta en el registry).
    pub version: String,
    /// Versión del ABI declarada por el plugin.
    pub abi_version: i32,
    /// Permisos solicitados (deny-by-default).
    #[serde(default)]
    pub permissions: Permissions,
    /// SHA-256 (hex) de los bytes wasm.
    pub sha256: String,
    /// Firma ed25519 (base64) sobre los bytes wasm en crudo.
    pub signature: String,
}

// ---------------------------------------------------------------------------
// Errores
// ---------------------------------------------------------------------------

/// Errores del host de plugins.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// La firma no es válida o no proviene de una clave de confianza.
    #[error("plugin sin firmar o de clave no confiable")]
    UnsignedOrUntrusted,

    /// El SHA-256 del manifiesto no coincide con el de los bytes wasm.
    #[error(
        "hash del manifiesto no coincide con los bytes wasm (esperado={expected}, real={actual})"
    )]
    HashMismatch {
        /// Hash declarado en el manifiesto.
        expected: String,
        /// Hash real calculado sobre los bytes wasm.
        actual: String,
    },

    /// La versión del ABI no coincide con [`ABI_VERSION`].
    #[error("ABI incompatible: esperado={expected}, encontrado={found}")]
    AbiMismatch {
        /// ABI soportado por el host.
        expected: i32,
        /// ABI reportado por el plugin (manifiesto o export).
        found: i32,
    },

    /// El hash del plugin está revocado en el registry curado.
    #[error("plugin revocado (sha256={0})")]
    Revoked(String),

    /// La versión solicitada no coincide con la del manifiesto (version pinning).
    #[error("versión no coincide: requerida={required}, manifiesto={found}")]
    VersionMismatch {
        /// Versión exigida por el llamador.
        required: String,
        /// Versión declarada en el manifiesto.
        found: String,
    },

    /// El plugin solicita permisos no soportados por el host (deny-by-default).
    #[error("permiso no soportado solicitado por el plugin: {0}")]
    PermissionDenied(&'static str),

    /// La firma base64 está mal formada.
    #[error("firma mal formada: {0}")]
    BadSignature(String),

    /// El plugin no exporta una función requerida con la firma esperada.
    #[error("export faltante o con firma incorrecta: {0}")]
    MissingExport(String),

    /// La ejecución agotó el *fuel* (probable bucle infinito).
    #[error("plugin agotó el fuel (posible bucle infinito)")]
    OutOfFuel,

    /// Error subyacente de wasmi (compilación, instanciación, trap, etc.).
    #[error("error de wasm: {0}")]
    Wasm(String),
}

// ---------------------------------------------------------------------------
// Helpers de firma
// ---------------------------------------------------------------------------

/// Calcula el SHA-256 de `bytes` y lo devuelve en hex (minúsculas).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Firma los bytes wasm en crudo con `signing_key` y devuelve la firma en base64.
pub fn sign(wasm: &[u8], signing_key: &SigningKey) -> String {
    let sig: Signature = signing_key.sign(wasm);
    B64.encode(sig.to_bytes())
}

/// Verifica la firma base64 sobre los bytes wasm contra la clave pública `vk`.
///
/// Devuelve `false` ante cualquier problema (base64 inválido, longitud
/// incorrecta o firma que no valida).
pub fn verify(wasm: &[u8], signature_b64: &str, vk: &VerifyingKey) -> bool {
    let raw = match B64.decode(signature_b64) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let sig = match Signature::from_slice(&raw) {
        Ok(s) => s,
        Err(_) => return false,
    };
    vk.verify(wasm, &sig).is_ok()
}

/// Verifica la firma contra una lista de claves de confianza. `true` si **alguna**
/// clave valida la firma.
fn verify_against_any(wasm: &[u8], signature_b64: &str, trusted: &[VerifyingKey]) -> bool {
    let raw = match B64.decode(signature_b64) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let sig = match Signature::from_slice(&raw) {
        Ok(s) => s,
        Err(_) => return false,
    };
    trusted.iter().any(|vk| vk.verify(wasm, &sig).is_ok())
}

// ---------------------------------------------------------------------------
// Host
// ---------------------------------------------------------------------------

/// Host de ejecución de plugins WASM con *fuel* habilitado y deny-by-default.
pub struct PluginHost {
    engine: Engine,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHost {
    /// Crea un nuevo host con un motor wasmi que consume *fuel* (metering).
    pub fn new() -> Self {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        Self { engine }
    }

    /// Carga un plugin **firmado**, aplicando todas las verificaciones de seguridad.
    ///
    /// Pasos (deny-by-default): valida el hash, valida la firma contra al menos
    /// una clave de confianza, rechaza permisos no soportados, instancia el
    /// módulo y comprueba el export `perfkit_abi_version()`.
    pub fn load_signed(
        &self,
        wasm: &[u8],
        manifest: &PluginManifest,
        trusted: &[VerifyingKey],
    ) -> Result<LoadedPlugin, PluginError> {
        // (0) deny-by-default: el MVP solo admite plugins puros (sin imports).
        if !manifest.permissions.is_pure() {
            let which = if manifest.permissions.allow_net {
                "allow_net"
            } else if manifest.permissions.allow_env {
                "allow_env"
            } else {
                "allow_fs"
            };
            return Err(PluginError::PermissionDenied(which));
        }

        // (0b) coherencia del ABI declarado en el manifiesto.
        if manifest.abi_version != ABI_VERSION {
            return Err(PluginError::AbiMismatch {
                expected: ABI_VERSION,
                found: manifest.abi_version,
            });
        }

        // (1) integridad: el hash del manifiesto debe coincidir con los bytes.
        let actual = sha256_hex(wasm);
        if !constant_time_eq(actual.as_bytes(), manifest.sha256.as_bytes()) {
            return Err(PluginError::HashMismatch {
                expected: manifest.sha256.clone(),
                actual,
            });
        }

        // (2) autenticidad: la firma debe validar contra alguna clave de confianza.
        if trusted.is_empty() || !verify_against_any(wasm, &manifest.signature, trusted) {
            return Err(PluginError::UnsignedOrUntrusted);
        }

        // (3) instanciación en sandbox: linker vacío => sin imports (deny-by-default).
        let module =
            Module::new(&self.engine, wasm).map_err(|e| PluginError::Wasm(e.to_string()))?;

        let mut store = Store::new(&self.engine, ());
        // Fuel para la instanciación (función start, si la hubiera).
        store
            .set_fuel(DEFAULT_FUEL)
            .map_err(|e| PluginError::Wasm(e.to_string()))?;

        let linker: Linker<()> = Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(map_wasm_err)?
            .start(&mut store)
            .map_err(map_wasm_err)?;

        // (4) handshake de ABI mediante el export del propio módulo.
        let abi_fn = instance
            .get_typed_func::<(), i32>(&store, "perfkit_abi_version")
            .map_err(|_| PluginError::MissingExport("perfkit_abi_version() -> i32".into()))?;
        let abi = abi_fn.call(&mut store, ()).map_err(map_wasm_err)?;
        if abi != ABI_VERSION {
            return Err(PluginError::AbiMismatch {
                expected: ABI_VERSION,
                found: abi,
            });
        }

        // Verificamos que `run` exista con la firma correcta de una vez.
        instance
            .get_typed_func::<i64, i64>(&store, "run")
            .map_err(|_| PluginError::MissingExport("run(i64) -> i64".into()))?;

        Ok(LoadedPlugin {
            store,
            instance,
            fuel_per_call: DEFAULT_FUEL,
            name: manifest.name.clone(),
            version: manifest.version.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Plugin cargado
// ---------------------------------------------------------------------------

/// Un plugin ya verificado e instanciado, listo para ejecutar `run`.
pub struct LoadedPlugin {
    store: Store<()>,
    instance: Instance,
    fuel_per_call: u64,
    name: String,
    version: String,
}

impl std::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPlugin")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("fuel_per_call", &self.fuel_per_call)
            .finish_non_exhaustive()
    }
}

impl LoadedPlugin {
    /// Nombre del plugin (del manifiesto).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Versión del plugin (del manifiesto).
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Ajusta el límite de *fuel* aplicado en cada llamada a [`Self::call_run`].
    pub fn set_fuel_per_call(&mut self, fuel: u64) {
        self.fuel_per_call = fuel;
    }

    /// Invoca el export `run(i64) -> i64` con un límite de *fuel*.
    ///
    /// Si el plugin agota el *fuel* (p.ej. un bucle infinito), devuelve
    /// `Err(PluginError::OutOfFuel)` en lugar de colgar el host.
    pub fn call_run(&mut self, input: i64) -> Result<i64, PluginError> {
        // Reponemos fuel antes de cada llamada para acotar *esta* invocación.
        self.store
            .set_fuel(self.fuel_per_call)
            .map_err(|e| PluginError::Wasm(e.to_string()))?;

        let run = self
            .instance
            .get_typed_func::<i64, i64>(&self.store, "run")
            .map_err(|_| PluginError::MissingExport("run(i64) -> i64".into()))?;

        run.call(&mut self.store, input).map_err(map_wasm_err)
    }
}

// ---------------------------------------------------------------------------
// Registry curado
// ---------------------------------------------------------------------------

/// Registry curado de plugins: claves de confianza, lista de revocación por
/// hash y carga con *version pinning*.
#[derive(Default)]
pub struct CuratedRegistry {
    trusted: Vec<VerifyingKey>,
    revoked: HashSet<String>,
}

impl CuratedRegistry {
    /// Crea un registry vacío (sin claves de confianza; deny-by-default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade una clave pública de confianza.
    pub fn add_trusted(&mut self, vk: VerifyingKey) {
        self.trusted.push(vk);
    }

    /// Revoca un plugin por su SHA-256 (hex). Un hash revocado nunca cargará.
    pub fn revoke(&mut self, sha256: impl Into<String>) {
        self.revoked.insert(sha256.into());
    }

    /// `true` si el hash está en la lista de revocación.
    pub fn is_revoked(&self, sha256: &str) -> bool {
        self.revoked.contains(sha256)
    }

    /// Claves de confianza registradas.
    pub fn trusted(&self) -> &[VerifyingKey] {
        &self.trusted
    }

    /// Carga un plugin firmado a través del host, rechazando además hashes
    /// revocados. Usa un [`PluginHost`] efímero (el motor wasmi es barato de crear).
    pub fn load(
        &self,
        wasm: &[u8],
        manifest: &PluginManifest,
    ) -> Result<LoadedPlugin, PluginError> {
        if self.revoked.contains(&manifest.sha256) {
            return Err(PluginError::Revoked(manifest.sha256.clone()));
        }
        let host = PluginHost::new();
        host.load_signed(wasm, manifest, &self.trusted)
    }

    /// Como [`Self::load`] pero exigiendo una versión exacta (*version pinning*).
    pub fn load_pinned(
        &self,
        wasm: &[u8],
        manifest: &PluginManifest,
        required_version: &str,
    ) -> Result<LoadedPlugin, PluginError> {
        if !version_matches(required_version, &manifest.version) {
            return Err(PluginError::VersionMismatch {
                required: required_version.to_string(),
                found: manifest.version.clone(),
            });
        }
        self.load(wasm, manifest)
    }
}

/// Helper de *version pinning*: coincidencia exacta de versión.
pub fn version_matches(required: &str, actual: &str) -> bool {
    required == actual
}

// ---------------------------------------------------------------------------
// Utilidades internas
// ---------------------------------------------------------------------------

/// Comparación en tiempo constante para evitar fugas por temporización al
/// comparar hashes. Devuelve `false` si difieren en longitud o contenido.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Traduce un [`wasmi::Error`] a [`PluginError`], detectando el agotamiento de
/// *fuel* (`TrapCode::OutOfFuel`) como variante propia.
fn map_wasm_err(e: wasmi::Error) -> PluginError {
    if let Some(code) = e.as_trap_code()
        && matches!(code, TrapCode::OutOfFuel)
    {
        return PluginError::OutOfFuel;
    }
    PluginError::Wasm(e.to_string())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Semilla fija de 32 bytes => keypair determinista (sin OsRng, sin flakiness).
    fn keypair(seed: u8) -> (SigningKey, VerifyingKey) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    /// WAT de un plugin válido: ABI=1 y `run(x) = x * 2`.
    fn valid_wat() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
              (func (export "perfkit_abi_version") (result i32)
                (i32.const 1))
              (func (export "run") (param i64) (result i64)
                (i64.mul (local.get 0) (i64.const 2))))
            "#,
        )
        .expect("WAT válido debe compilar")
    }

    /// WAT con ABI incorrecto (=2).
    fn wrong_abi_wat() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
              (func (export "perfkit_abi_version") (result i32)
                (i32.const 2))
              (func (export "run") (param i64) (result i64)
                (local.get 0)))
            "#,
        )
        .expect("WAT válido debe compilar")
    }

    /// WAT con `run` en bucle infinito.
    fn infinite_loop_wat() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
              (func (export "perfkit_abi_version") (result i32)
                (i32.const 1))
              (func (export "run") (param i64) (result i64)
                (loop $l (br $l))
                (i64.const 0)))
            "#,
        )
        .expect("WAT válido debe compilar")
    }

    /// Construye un manifiesto correcto (hash + firma) para `wasm`.
    fn manifest_for(
        wasm: &[u8],
        sk: &SigningKey,
        name: &str,
        version: &str,
        abi: i32,
    ) -> PluginManifest {
        PluginManifest {
            name: name.to_string(),
            version: version.to_string(),
            abi_version: abi,
            permissions: Permissions::default(),
            sha256: sha256_hex(wasm),
            signature: sign(wasm, sk),
        }
    }

    #[test]
    fn sha256_hex_known_vector() {
        // SHA-256("") = e3b0c442...855
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let (sk, vk) = keypair(7);
        let wasm = valid_wat();
        let sig = sign(&wasm, &sk);
        assert!(verify(&wasm, &sig, &vk));

        // Firma válida pero clave distinta => falla.
        let (_, vk2) = keypair(8);
        assert!(!verify(&wasm, &sig, &vk2));

        // base64 mal formado => falla sin pánico.
        assert!(!verify(&wasm, "no-es-base64!!!", &vk));
    }

    #[test]
    fn valid_signed_plugin_loads_and_runs() {
        let (sk, vk) = keypair(1);
        let wasm = valid_wat();
        let manifest = manifest_for(&wasm, &sk, "doubler", "1.0.0", ABI_VERSION);

        let host = PluginHost::new();
        let mut plugin = host
            .load_signed(&wasm, &manifest, &[vk])
            .expect("plugin firmado y confiable debe cargar");

        assert_eq!(plugin.name(), "doubler");
        assert_eq!(plugin.version(), "1.0.0");
        assert_eq!(plugin.call_run(21).unwrap(), 42);
        // Idempotente entre llamadas (fuel se repone).
        assert_eq!(plugin.call_run(10).unwrap(), 20);
        assert_eq!(plugin.call_run(-5).unwrap(), -10);
    }

    #[test]
    fn tampered_wasm_fails_to_load() {
        let (sk, vk) = keypair(2);
        let wasm = valid_wat();
        // Manifiesto firmado sobre el wasm ORIGINAL.
        let manifest = manifest_for(&wasm, &sk, "doubler", "1.0.0", ABI_VERSION);

        // Manipulamos un byte del wasm (sin recalcular hash/firma).
        let mut tampered = wasm.clone();
        let idx = tampered.len() / 2;
        tampered[idx] ^= 0xFF;

        let host = PluginHost::new();
        let err = host
            .load_signed(&tampered, &manifest, &[vk])
            .expect_err("wasm manipulado NO debe cargar");
        // El hash ya no coincide (primera barrera).
        assert!(
            matches!(err, PluginError::HashMismatch { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn tampered_wasm_with_matching_hash_still_fails_signature() {
        // Caso más fuerte: atacante recalcula el hash del wasm manipulado pero
        // NO puede re-firmar (no tiene la clave privada). La firma debe fallar.
        let (sk, vk) = keypair(2);
        let wasm = valid_wat();
        let good_sig = sign(&wasm, &sk);

        let mut tampered = wasm.clone();
        let mid = tampered.len() / 2;
        tampered[mid] ^= 0xFF;

        let manifest = PluginManifest {
            name: "evil".into(),
            version: "1.0.0".into(),
            abi_version: ABI_VERSION,
            permissions: Permissions::default(),
            sha256: sha256_hex(&tampered), // hash recalculado => coincide
            signature: good_sig,           // firma vieja del wasm original
        };

        let host = PluginHost::new();
        let err = host
            .load_signed(&tampered, &manifest, &[vk])
            .expect_err("firma inválida sobre wasm manipulado NO debe cargar");
        assert!(
            matches!(err, PluginError::UnsignedOrUntrusted),
            "got {err:?}"
        );
    }

    #[test]
    fn untrusted_key_fails_to_load() {
        let (signer, _signer_vk) = keypair(3);
        let (_other, trusted_vk) = keypair(4); // clave de confianza distinta
        let wasm = valid_wat();
        // Firmado por `signer`, pero confiamos en `trusted_vk`.
        let manifest = manifest_for(&wasm, &signer, "doubler", "1.0.0", ABI_VERSION);

        let host = PluginHost::new();
        let err = host
            .load_signed(&wasm, &manifest, &[trusted_vk])
            .expect_err("clave no confiable NO debe cargar");
        assert!(
            matches!(err, PluginError::UnsignedOrUntrusted),
            "got {err:?}"
        );
    }

    #[test]
    fn empty_trust_set_fails_to_load() {
        let (sk, _vk) = keypair(5);
        let wasm = valid_wat();
        let manifest = manifest_for(&wasm, &sk, "doubler", "1.0.0", ABI_VERSION);

        let host = PluginHost::new();
        let err = host
            .load_signed(&wasm, &manifest, &[])
            .expect_err("sin claves de confianza NO debe cargar");
        assert!(
            matches!(err, PluginError::UnsignedOrUntrusted),
            "got {err:?}"
        );
    }

    #[test]
    fn wrong_abi_export_is_rejected() {
        let (sk, vk) = keypair(6);
        let wasm = wrong_abi_wat();
        // El manifiesto declara ABI=1 (válido) pero el export devuelve 2.
        let manifest = manifest_for(&wasm, &sk, "bad-abi", "1.0.0", ABI_VERSION);

        let host = PluginHost::new();
        let err = host
            .load_signed(&wasm, &manifest, &[vk])
            .expect_err("ABI del export incorrecto NO debe cargar");
        assert!(
            matches!(
                err,
                PluginError::AbiMismatch {
                    expected: 1,
                    found: 2
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn manifest_abi_mismatch_is_rejected() {
        let (sk, vk) = keypair(6);
        let wasm = valid_wat();
        // Manifiesto declara ABI=2 aunque el export sea 1.
        let manifest = manifest_for(&wasm, &sk, "bad-manifest-abi", "1.0.0", 2);

        let host = PluginHost::new();
        let err = host
            .load_signed(&wasm, &manifest, &[vk])
            .expect_err("ABI del manifiesto incorrecto NO debe cargar");
        assert!(
            matches!(
                err,
                PluginError::AbiMismatch {
                    expected: 1,
                    found: 2
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn permissions_requested_are_denied() {
        let (sk, vk) = keypair(9);
        let wasm = valid_wat();
        let mut manifest = manifest_for(&wasm, &sk, "needs-net", "1.0.0", ABI_VERSION);
        manifest.permissions.allow_net = true;

        let host = PluginHost::new();
        let err = host
            .load_signed(&wasm, &manifest, &[vk])
            .expect_err("permiso no soportado NO debe cargar");
        assert!(
            matches!(err, PluginError::PermissionDenied("allow_net")),
            "got {err:?}"
        );
    }

    #[test]
    fn infinite_loop_run_returns_out_of_fuel_without_hanging() {
        let (sk, vk) = keypair(10);
        let wasm = infinite_loop_wat();
        let manifest = manifest_for(&wasm, &sk, "looper", "1.0.0", ABI_VERSION);

        let host = PluginHost::new();
        let mut plugin = host
            .load_signed(&wasm, &manifest, &[vk])
            .expect("el plugin instancia bien; el bucle está en run");

        let err = plugin
            .call_run(0)
            .expect_err("bucle infinito debe agotar el fuel");
        assert!(matches!(err, PluginError::OutOfFuel), "got {err:?}");

        // El host sigue vivo: no se colgó. (Si llegamos aquí, no hubo hang.)
    }

    #[test]
    fn registry_loads_valid_plugin() {
        let (sk, vk) = keypair(11);
        let wasm = valid_wat();
        let manifest = manifest_for(&wasm, &sk, "doubler", "2.3.4", ABI_VERSION);

        let mut reg = CuratedRegistry::new();
        reg.add_trusted(vk);

        let mut plugin = reg.load(&wasm, &manifest).expect("registry debe cargar");
        assert_eq!(plugin.call_run(21).unwrap(), 42);
    }

    #[test]
    fn registry_rejects_revoked_hash() {
        let (sk, vk) = keypair(12);
        let wasm = valid_wat();
        let manifest = manifest_for(&wasm, &sk, "doubler", "1.0.0", ABI_VERSION);

        let mut reg = CuratedRegistry::new();
        reg.add_trusted(vk);
        reg.revoke(manifest.sha256.clone());
        assert!(reg.is_revoked(&manifest.sha256));

        let err = reg
            .load(&wasm, &manifest)
            .expect_err("hash revocado NO debe cargar");
        assert!(matches!(err, PluginError::Revoked(_)), "got {err:?}");
    }

    #[test]
    fn registry_version_pinning_mismatch_is_rejected() {
        let (sk, vk) = keypair(13);
        let wasm = valid_wat();
        let manifest = manifest_for(&wasm, &sk, "doubler", "1.0.0", ABI_VERSION);

        let mut reg = CuratedRegistry::new();
        reg.add_trusted(vk);

        // Pedimos 2.0.0 pero el manifiesto es 1.0.0 => mismatch.
        let err = reg
            .load_pinned(&wasm, &manifest, "2.0.0")
            .expect_err("version pinning no coincide => Err");
        assert!(
            matches!(err, PluginError::VersionMismatch { .. }),
            "got {err:?}"
        );

        // Versión exacta => OK.
        let mut plugin = reg
            .load_pinned(&wasm, &manifest, "1.0.0")
            .expect("version pinning exacto debe cargar");
        assert_eq!(plugin.call_run(3).unwrap(), 6);
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let (sk, _vk) = keypair(14);
        let wasm = valid_wat();
        let manifest = manifest_for(&wasm, &sk, "doubler", "1.0.0", ABI_VERSION);

        let json = serde_json::to_string(&manifest).unwrap();
        let back: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, back);
        // Permissions por defecto = todo false.
        assert!(back.permissions.is_pure());
    }
}
