use std::io::Write as _;
use std::path::Path;
use std::process::Child;

use fs2::FileExt as _;

use super::*;

struct CollectionChild(Option<Child>);

impl CollectionChild {
    fn spawn(database: &str) -> Result<Self> {
        let mut child = Command::new(BINARY)
            .args(["brief", "--db", database])
            .env_remove("SIFTWIRE_DATABASE_PATH")
            .env("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut input = child.stdin.take().context("collection stdin")?;
        let collection = Self(Some(child));
        input.write_all(b"{\"action\":\"run_brief\"}")?;
        drop(input);
        Ok(collection)
    }

    fn writer(&mut self, path: &Path) -> Result<fs::File> {
        let started = std::time::Instant::now();
        loop {
            match rustix::fs::open(
                path,
                rustix::fs::OFlags::WRONLY
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            ) {
                Ok(file) => return Ok(file.into()),
                Err(rustix::io::Errno::NXIO) => {}
                Err(error) => return Err(error).context("open fixture writer"),
            }
            if self
                .0
                .as_mut()
                .context("collection child")?
                .try_wait()?
                .is_some()
            {
                let output = self
                    .0
                    .take()
                    .context("collection child")?
                    .wait_with_output()?;
                anyhow::bail!("collection exited before fetching: {output:?}");
            }
            anyhow::ensure!(
                started.elapsed() < std::time::Duration::from_mins(1),
                "fixture fetch rendezvous stalled"
            );
            std::thread::yield_now();
        }
    }

    fn finish(mut self) -> Result<serde_json::Value> {
        let output = self
            .0
            .take()
            .context("collection child")?
            .wait_with_output()?;
        assert!(output.status.success(), "collection failed: {output:?}");
        serde_json::from_slice(&output.stdout).context("collection result")
    }
}

impl Drop for CollectionChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _killed = child.kill();
            let _waited = child.wait();
        }
    }
}

fn configure(database: &str, feed: &Path) -> Result<()> {
    let url = url::Url::from_file_path(feed).map_err(|()| anyhow::anyhow!("fixture URL"))?;
    invoke_json(
        &["config", "--db", database],
        &json!({"action":"upsert_source","source":{"key":"feed","label":"Fixture","kind":"rss","url":url,"section":"technology","threshold":"always","enabled":true}}),
        &BTreeMap::from([("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1")]),
    )?;
    Ok(())
}

fn feed(title: &str) -> String {
    format!(
        "<rss version=\"2.0\"><channel><title>Fixture</title><item><title>{title}</title><guid>{title}</guid><link>https://example.test/{title}</link></item></channel></rss>"
    )
}

#[test]
fn overlapping_collections_do_not_repeat_or_regress_required_markers() -> Result<()> {
    for second_title in ["old", "new"] {
        let temp = TempDir::new()?;
        let database = temp.path().join("fixture.sqlite");
        let database_text = database.to_string_lossy().into_owned();
        let fifo = temp.path().join("feed.fifo");
        assert!(Command::new("mkfifo").arg(&fifo).status()?.success());
        configure(&database_text, &fifo)?;
        let mut first = CollectionChild::spawn(&database_text)?;
        // The writer rendezvous proves the first process is fetching, not just spawned.
        let mut writer = first.writer(&fifo)?;
        let probe = fs::File::open(&database)?;
        assert_eq!(
            probe.try_lock_exclusive().err().map(|error| error.kind()),
            Some(std::io::ErrorKind::WouldBlock),
            "durable fetch did not hold the database collection lock"
        );
        drop(probe);
        let next_feed = temp.path().join("next.fifo");
        assert!(Command::new("mkfifo").arg(&next_feed).status()?.success());
        configure(&database_text, &next_feed)?;
        let alias = temp.path().join("alias.sqlite");
        std::os::unix::fs::symlink(&database, &alias)?;
        let mut second = CollectionChild::spawn(&alias.to_string_lossy())?;
        writer.write_all(feed("old").as_bytes())?;
        drop(writer);
        let first_result = first.finish()?;
        let mut writer = second.writer(&next_feed)?;
        let probe = fs::File::open(&database)?;
        assert_eq!(
            probe.try_lock_exclusive().err().map(|error| error.kind()),
            Some(std::io::ErrorKind::WouldBlock),
            "alias collection did not lock the database inode"
        );
        drop(probe);
        writer.write_all(feed(second_title).as_bytes())?;
        drop(writer);
        let second_result = second.finish()?;
        assert_eq!(
            first_result.pointer("/must_include/0/title"),
            Some(&json!("old"))
        );
        if second_title == "old" {
            assert!(
                second_result.get("must_include").is_none(),
                "overlap reused the Required marker: {second_result}"
            );
        } else {
            assert_eq!(
                second_result.pointer("/must_include/0/title"),
                Some(&json!("new"))
            );
        }
        let db = rusqlite::Connection::open(&database)?;
        let marker: String = db.query_row("SELECT latest_title FROM source_state", [], |row| {
            row.get(0)
        })?;
        assert_eq!(
            marker, second_title,
            "older collection regressed the marker"
        );
        drop(db);
        let probe = fs::File::open(&database)?;
        probe
            .try_lock_exclusive()
            .context("finished collections retained their lock")?;
    }
    Ok(())
}

#[test]
fn interrupted_collection_releases_database_guard() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp.path().join("fixture.sqlite");
    let database_text = database.to_string_lossy().into_owned();
    let fifo = temp.path().join("feed.fifo");
    assert!(Command::new("mkfifo").arg(&fifo).status()?.success());
    configure(&database_text, &fifo)?;
    let mut child = CollectionChild::spawn(&database_text)?;
    let writer = child.writer(&fifo)?;
    let probe = fs::File::open(&database)?;
    assert_eq!(
        probe.try_lock_exclusive().err().map(|error| error.kind()),
        Some(std::io::ErrorKind::WouldBlock),
        "collection lock missing"
    );
    drop(child);
    drop(writer);
    probe
        .try_lock_exclusive()
        .context("interrupted collection retained its lock")?;
    let mut failed = CollectionChild::spawn(&temp.path().to_string_lossy())?;
    let error = failed
        .writer(&fifo)
        .err()
        .context("failed child reached fetching")?;
    assert!(
        error
            .to_string()
            .contains("collection exited before fetching"),
        "missing early failure: {error:#}"
    );
    Ok(())
}
