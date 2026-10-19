//! Bounded focus discovery for fields that emit no signal on a repeated click.
//! Reads state, hierarchy and caret availability only, never names or text.
use super::{Connection, OwnedObjectPath, proxy};
use std::collections::VecDeque;

pub(super) async fn find(connection: &Connection) -> Option<(String, String)> {
    let root = proxy(
        connection,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/accessible/root",
        "org.a11y.atspi.Accessible",
    )
    .await
    .ok()?;
    let apps: Vec<(String, OwnedObjectPath)> = root.call("GetChildren", &()).await.ok()?;
    let mut pending: VecDeque<_> = apps.into_iter().collect();
    let mut visited = std::collections::HashSet::new();
    for _ in 0..512 {
        let (bus, path) = pending.pop_back()?;
        if path.as_str().ends_with("/null") || !visited.insert((bus.clone(), path.clone())) {
            continue;
        }
        let Ok(object) = proxy(connection, &bus, path.as_str(), "org.a11y.atspi.Accessible").await
        else {
            continue;
        };
        let states: Vec<u32> = object.call("GetState", &()).await.unwrap_or_default();
        let flags = states.first().copied().unwrap_or(0);
        if flags & (1 << 12) != 0 && flags & (1 << 25) != 0 && flags & (1 << 30) != 0 {
            if let Ok(text) = proxy(connection, &bus, path.as_str(), "org.a11y.atspi.Text").await {
                if text
                    .get_property::<i32>("CaretOffset")
                    .await
                    .is_ok_and(|offset| offset >= 0)
                {
                    return Some((bus, path.to_string()));
                }
            }
        }
        if let Ok(children) = object
            .call::<_, _, Vec<(String, OwnedObjectPath)>>("GetChildren", &())
            .await
        {
            pending.extend(
                children
                    .into_iter()
                    .filter(|(child_bus, _)| child_bus == &bus),
            );
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore = "requires TARANTINO_ATSPI_EXPECTED_BUS and a focused external text field"]
    async fn native_external_focused_field() {
        let expected = std::env::var("TARANTINO_ATSPI_EXPECTED_BUS").unwrap();
        let connection = super::super::connect().await.unwrap();
        let target = tokio::time::timeout(
            std::time::Duration::from_millis(700),
            super::find(&connection),
        )
        .await
        .unwrap()
        .expect("focused field");
        assert_eq!(target.0, expected);
        println!("Focused accessible: {target:?}");
    }
}
