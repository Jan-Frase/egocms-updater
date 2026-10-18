# EgoCMS REST API: working with files

Verified against the EgoCMS instance from `egocms/` (source in `lib/base/Page.php` and `lib/base/Ego_REST_Server.php`)
and by calling the API with `curl`. The official docs barely cover this.

## Key concepts

### Files live on the *media site*, not on the content site

Every content site (e.g. `materialkit`) is paired with a media site (`materialkit_media`, set via `media=` in
`site/materialkit/conf.ini`). Files and images are pages of type `multimedia/file` / `multimedia/image` that are
children of a `multimedia/category` page *in the media site*.

`Page::newFile` does nothing unless the page it is called on belongs to a site of type `media`:

```php
if ($this->_site->site['type'] == 'media' && $this->validateFile($source, $name)) { ... }
return null;
```

**Calling `POST /rest/materialkit/de/56/newFile` therefore returns `201` but uploads nothing.** The server sends `201`
for every `POST` that does not throw, regardless of whether the method returned `null`.
Always call file endpoints on `/rest/<site>_media/<lang>/<folder_id>/...`.

Find the media site with `GET /rest/materialkit/de/getMediaSite`. Its root page (id `1`, "Homepage", type
`multimedia/category`) is a valid parent. Create sub-folders with `newChild` (see below).

### Status codes

| Method | Success | Failure |
|--------|---------|---------|
| GET    | 200     | 204 (empty) |
| POST   | 201     | 409 (exception) |
| PUT    | 202     | 409 |
| DELETE | 204     | 400 |

Success only means "no exception". Check the response body: `newFile` returns the new page (`{"field":…,"extra":…}`),
and returns **`null`/empty** when it silently refused (wrong site, virus scan failed, file type not whitelisted, image
too small...). Treat an empty or `null` body as failure.

### The multipart upload format

The REST server (`Ego_REST_Server::__construct`) reads the positional arguments of the PHP method from a JSON array in
the form field `@data`. Any argument equal to the string `"@file"` is replaced by the temp path of the uploaded file.

```
POST /rest/materialkit_media/de/<parent_id>/newFile
Content-Type: multipart/form-data

@data  = ["@file", "report.pdf", {"title": "My Report"}]
upload = <binary file>        (the form field name is arbitrary; the first uploaded file is used)
```

The arguments map onto `newFile($source, $name, $options = [], $suffix = '')`.

Authentication is the same as for all other calls (session cookie from `startSession`, or
`Authorization: Bearer <user_id>.<token>`). The user needs the `child` right on the parent folder.

---

## Endpoints

All of these exist on `Page` and are exposed in `lib/base/rest.ini`. Base URL: `/rest/<media_site>/<lang>/<page_id>/<method>`.
Required rights are in brackets.

### `newFile` (POST, `child`): upload a new file

Creates a child page of `<page_id>` containing the file.

| Arg | Description |
|-----|-------------|
| `source` | Always `"@file"` |
| `name` | File name including extension, e.g. `report.pdf`. Used as page `name`. The extension determines `image_type` (falls back to MIME detection) |
| `options` | Optional page fields. `title` (defaults to name), `name` (overrides the name), `extra` (object merged into the page's extra, e.g. `mime_type`) |
| `suffix` | Optional: force the file extension |

```bash
curl -k -b cookies.txt \
  -F '@data=["@file","report.pdf",{"title":"My Report"}]' \
  -F 'upload=@report.pdf' \
  https://localhost/rest/materialkit_media/de/1/newFile
```

Returns the new page: `field.id` is the id of the file page. `extra` contains `mime_type`, `image_type`,
`origFileSize` (and `origImgWidth` / `origImgHeight` for images). Files are stored under `var/media/<site>/`.
Images are additionally scaled and converted in the background (queue).

**Not idempotent:** uploading the same name twice creates two pages. Use `hasFile` first.

### `hasFile` (GET, `view`): look up a file by name

`GET /rest/materialkit_media/de/<parent_id>/hasFile?name=report.pdf[&suffix=pdf]`

Returns the child page (`field`/`extra`) with that name directly below `<parent_id>`, or `null` if none exists.
Only direct children of that page are searched.

### `updateFile` (POST, `edit`): replace the content of an existing file

`POST /rest/materialkit_media/de/<file_page_id>/updateFile`, body: `@data = ["@file", "report.pdf"]` + the binary.

`<file_page_id>` is the *file's* page id (from `newFile` / `hasFile`), not the parent. The second argument (`name`) is
optional and only used to determine the extension. Returns `true` on success, `false` if validation failed. The title
and name of the page are kept. A history copy of the previous version is archived.

### `validateFile` (POST, `edit`): dry-run the upload checks

Same body format as `updateFile`. Returns whether the file passes the site's `files` configuration (virus scan,
whitelist/blacklist of extensions, size limits). `newFile` and `updateFile` call this internally and fail silently when
it returns false, so use this to find out *why* an upload produced nothing.

### `getUrl` (GET, `view`): public URL of the file

`GET /rest/materialkit_media/de/<file_page_id>/getUrl` -> `"/materialkit_media/report_pdf.pdf"`

Prefix with the host to get the link. Alternatively `GET /rest/materialkit/de/getMediaUrl?id=<file_page_id>` on the
content site.

### `newChild` (POST, `child`): create a folder

Use it on the media site to create a category to hold files. Body `@data` / JSON: `field` with `type` =
`multimedia/category` and a `title`/`name`. See the existing `new_child` in `src/types/communicator.rs`, the media
site uses the same call with a different type.

### Other related calls (see "Verfügbare Methoden")

* `Page.extractFile`: unpack a ZIP into child pages (not exposed by default in `rest.ini`).
* `Pool.put / get / list / move / copy / delete` (`.../<id>/pool/<method>`): the media pool API.
* `Site.getMediaSite`, `Site.hasMediaSite`, `Site.getMediaUrl`.

---

## Deleting files

Yes, files are pages, so the normal page deletion applies.

| Call | Effect |
|------|--------|
| `DELETE /rest/materialkit_media/de/<file_id>/delete` (`remove`) | Moves the file to the trash (soft delete, `204`). The page then disappears from `getPage` (returns `null`) |
| `PUT /rest/materialkit_media/de/<file_id>/undelete` (`remove`) | Restores it from the trash. Currently untested; in my test it returned 409 `Invalid parameters` for a trashed page, because REST resolves the page without deleted pages |
| `POST /rest/materialkit_media/de/clearTrashcan` | Permanently removes **everything** in the trash of that site (`201`, "Der Papierkorb wurde geleert.") |
| `DELETE .../<file_id>/destroy` (`remove`) | Returns `204`, but in testing the page was still there afterwards. Don't rely on it |

Practical recipe to remove one file permanently: `DELETE …/<id>/delete` followed by `POST …/clearTrashcan` (this also purges anything else in that trash). Note that `delete` can answer `500` if the session expired; check the status.

There is no "delete by name" call: use `hasFile` to resolve the id first.

