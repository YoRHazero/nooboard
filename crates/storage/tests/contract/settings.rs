use super::*;
pub async fn exercise(storage: &Storage) {
    let settings = storage.settings();
    assert_eq!(settings.get("missing").await.unwrap(), None);
    settings
        .apply(SettingsChanges {
            put: vec![Setting {
                key: "old".into(),
                value: " 中文\r\n ".into(),
            }],
            delete: vec![],
        })
        .await
        .unwrap();
    settings
        .apply(SettingsChanges {
            put: vec![Setting {
                key: "new".into(),
                value: "{\"enabled\":true}".into(),
            }],
            delete: vec!["old".into()],
        })
        .await
        .unwrap();
    assert_eq!(
        settings
            .get_many(vec!["old".into(), "new".into(), "new".into()])
            .await
            .unwrap(),
        vec![
            None,
            Some("{\"enabled\":true}".into()),
            Some("{\"enabled\":true}".into())
        ]
    );
    let rejected = SettingsChanges {
        put: vec![Setting {
            key: "new".into(),
            value: "uncommitted".into(),
        }],
        delete: vec!["new".into()],
    };
    assert_eq!(
        settings.apply(rejected).await.unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        settings.get("new").await.unwrap().as_deref(),
        Some("{\"enabled\":true}")
    );
    assert_eq!(
        settings.get("").await.unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert!(settings.get_many(vec![]).await.unwrap().is_empty());
    settings.apply(SettingsChanges::default()).await.unwrap();
    assert_eq!(
        settings
            .apply(SettingsChanges {
                put: vec![Setting {
                    key: "bad".into(),
                    value: "a\0b".into()
                }],
                delete: vec![]
            })
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidInput
    );
}
