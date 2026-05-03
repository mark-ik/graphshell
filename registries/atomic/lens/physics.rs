// Slice 66: body moved to register-lens. Re-export shim.
pub(crate) use register_lens::{
    canonical_physics_profile_id_hint, physics_profile_descriptors, resolve_physics_profile,
    PhysicsProfile, PhysicsProfileResolution, PHYSICS_ID_DEFAULT, PHYSICS_ID_DRIFT,
    PHYSICS_ID_SCATTER, PHYSICS_ID_SETTLE,
};
