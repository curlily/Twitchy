//! commands/mod.rs
//!
//! Automatically declares command modules, brings their DESCRIPTORs into scope,
//! and creates a static COMMANDS array for runtime lookup.
/// Macro to declare modules and build the COMMANDS array
macro_rules! commands {
    ( $( $mod_name:ident ),* $(,)? ) => {
        $(
            pub mod $mod_name; // declare module
        )*

        /// Static array of all registered command descriptors
        pub const COMMANDS: &[&crate::command_registry::CommandDescriptor] = &[
            $( &$mod_name::DESCRIPTOR ),*
        ];
    };
}

// ── Register all commands here ──
// Add new commands by adding their name to this list
commands![
    hello,
    dice,
];
