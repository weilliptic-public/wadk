pub const DEFAULT_RETENTION_BYTES: u64 = 1000;

#[macro_export]
macro_rules! emit {
    ($event:ident($($field:ident $(: $value:expr)?),* $(,)?)) => {{
        match $event::new($(emit!(@value $field $($value)?)),*) {
            Ok(event) => event.emit(),
            Err(err) => Err(err),
        }
    }};
    (@value $field:ident $value:expr) => {
        $value
    };
    (@value $field:ident) => {
        $field
    };
}
