/// Merge Tailwind CSS classes with conflict resolution.
///
/// Provides a `cn()` utility similar to shadcn/ui's class merging function.
/// Later classes override earlier ones when they target the same CSS property.
#[must_use]
pub fn cn(classes: &[&str]) -> String {
    tailwind_fuse::tw_merge!(classes.join(" "))
}
