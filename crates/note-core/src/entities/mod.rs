/// Set `created_at` on insert (unless already set) and `updated_at` on every save.
/// Done in code rather than with DB defaults/triggers so it works on every backend.
macro_rules! touch_timestamps {
    ($model:expr, $insert:expr) => {{
        let mut m = $model;
        let now = ::chrono::Utc::now();
        if $insert && m.created_at.is_not_set() {
            m.created_at = ::sea_orm::ActiveValue::Set(now);
        }
        m.updated_at = ::sea_orm::ActiveValue::Set(now);
        m
    }};
}
pub(crate) use touch_timestamps;

pub mod users;
pub mod workspaces;
pub mod documents;
pub mod blocks;
