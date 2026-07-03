use std::path::{Path, PathBuf};

pub fn resolve_attachment_path(db_root: &Path, uuid: &str, target: &str) -> PathBuf {
    if uuid.len() > 2 {
        db_root
            .join(".attach")
            .join(&uuid[..2])
            .join(&uuid[2..])
            .join(target)
    } else {
        db_root.join(".attach").join(uuid).join(target)
    }
}

pub fn attachment_target_exists(db_root: &Path, uuid: &str, target: &str) -> bool {
    resolve_attachment_path(db_root, uuid, target).exists()
        || db_root.join(".attach").join(uuid).join(target).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_bucketed_attachment_path_for_uuid() {
        assert_eq!(
            resolve_attachment_path(
                Path::new("/db"),
                "11111111-1111-4111-8111-111111111111",
                "image.png",
            ),
            PathBuf::from("/db/.attach/11/111111-1111-4111-8111-111111111111/image.png")
        );
    }

    #[test]
    fn resolves_short_ids_without_bucket() {
        assert_eq!(
            resolve_attachment_path(Path::new("/db"), "ab", "image.png"),
            PathBuf::from("/db/.attach/ab/image.png")
        );
    }
}
