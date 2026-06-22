//! RBAC (control de acceso basado en roles), **deny-by-default**.
//!
//! La política se expresa de forma explícita en [`can`]: cualquier combinación
//! rol/acción no listada como permitida queda denegada. No hay comodines ni
//! herencia implícita; añadir un permiso requiere editar la matriz.
//!
//! # Matriz de permisos
//!
//! | Acción \ Rol      | Viewer | Operator | Admin |
//! |-------------------|:------:|:--------:|:-----:|
//! | `ViewRuns`        |   ✅   |    ✅    |  ✅   |
//! | `RunTest`         |   ❌   |    ✅    |  ✅   |
//! | `Annotate`        |   ❌   |    ✅    |  ✅   |
//! | `SetBaseline`     |   ❌   |    ❌    |  ✅   |
//! | `ManageRetention` |   ❌   |    ❌    |  ✅   |
//!
//! - **Viewer**: solo lectura (`ViewRuns`).
//! - **Operator**: lectura + ejecutar pruebas + anotar.
//! - **Admin**: todo.

use serde::{Deserialize, Serialize};

/// Rol asignado a un actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Solo lectura.
    Viewer,
    /// Lectura + ejecución + anotaciones.
    Operator,
    /// Acceso total.
    Admin,
}

/// Acción que un actor puede intentar realizar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Consultar runs / tendencias / comparaciones.
    ViewRuns,
    /// Lanzar una prueba.
    RunTest,
    /// Fijar/actualizar una baseline.
    SetBaseline,
    /// Añadir una anotación a un run.
    Annotate,
    /// Gestionar retención (purga de datos antiguos).
    ManageRetention,
}

/// Decide si `role` puede ejecutar `action`. Deny-by-default.
///
/// Ver la matriz a nivel de módulo. Cualquier par no contemplado devuelve `false`.
pub fn can(role: Role, action: Action) -> bool {
    use Action::*;
    use Role::*;
    match role {
        // Viewer: únicamente lectura.
        Viewer => matches!(action, ViewRuns),
        // Operator: lectura, ejecución y anotaciones.
        Operator => matches!(action, ViewRuns | RunTest | Annotate),
        // Admin: todas las acciones.
        Admin => matches!(
            action,
            ViewRuns | RunTest | SetBaseline | Annotate | ManageRetention
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_is_read_only() {
        assert!(can(Role::Viewer, Action::ViewRuns));
        assert!(!can(Role::Viewer, Action::SetBaseline));
        assert!(!can(Role::Viewer, Action::RunTest));
        assert!(!can(Role::Viewer, Action::Annotate));
        assert!(!can(Role::Viewer, Action::ManageRetention));
    }

    #[test]
    fn operator_can_run_and_annotate_but_not_baseline() {
        assert!(can(Role::Operator, Action::ViewRuns));
        assert!(can(Role::Operator, Action::RunTest));
        assert!(can(Role::Operator, Action::Annotate));
        assert!(!can(Role::Operator, Action::SetBaseline));
        assert!(!can(Role::Operator, Action::ManageRetention));
    }

    #[test]
    fn admin_can_everything() {
        for action in [
            Action::ViewRuns,
            Action::RunTest,
            Action::SetBaseline,
            Action::Annotate,
            Action::ManageRetention,
        ] {
            assert!(can(Role::Admin, action), "admin should allow {action:?}");
        }
    }
}
