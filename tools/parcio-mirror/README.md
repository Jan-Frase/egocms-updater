# Local parcio mirror

Two scripts to rebuild the public parcio website in the local EgoCMS test instance, in the state the editor leaves it:
no markers, links in the editor's `index.php?…` format, files attached to their pages. That allows testing the whole
migration (download, dry run, upload) locally. Both only need Python 3, no packages.

```bash
# 1. Once: save the public website into snapshot.json (slow on purpose, the site rate-limits).
python3 snapshot.py

# 2. After every reset of the containers: rebuild the mirror below the page `ParCIO`.
export EGOCMS_USER_ID=… EGOCMS_TOKEN=…
python3 seed.py

# 3. Test the updater against it, using the config seed.py wrote.
cd ../..
cargo run -- --config-path tools/parcio-mirror/mirror.toml download tools/parcio-mirror/pages
cargo run -- --config-path tools/parcio-mirror/mirror.toml upload --dry-run
```

`snapshot.py` can be interrupted and continues where it stopped; delete `snapshot.json` to start over.

Differences to the real site:

- The mirror is a subtree of the demo site (`site=materialkit`), its URLs don't end in `.html`.
- Attachments are small placeholder files with the real names.
- People pages get the demo types `testimonials/list` and `testimonials/entry`, publications `copyright`, so the
  updater skips them, as it should on the real site. All other pages are a single text block. The real page types are
  unknown, see `page_type` in `seed.py`.
- Links to pages outside the mirror and files of the media site point to the real website.
