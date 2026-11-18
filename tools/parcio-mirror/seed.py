#!/usr/bin/env python3
"""Rebuilds the parcio website from `snapshot.json` in a local EgoCMS, the way the editor would have
made it: no markers, links in the editor's `index.php?…` format and files attached to their pages.

All pages are created below a new page (`ParCIO` by default), which replaces an existing one of the
same name, so the script can be run again after the containers were reset. Attachments are small
placeholder files with the real names. At the end, `mirror.toml` is written: a config for running
the updater against the mirror.

Only uses the Python standard library:
    export EGOCMS_USER_ID=… EGOCMS_TOKEN=…
    python3 seed.py
"""

import argparse
import html
import http.cookiejar
import json
import os
import re
import ssl
import struct
import urllib.parse
import urllib.request
import uuid
import zlib
from pathlib import Path

HERE = Path(__file__).parent
BASE = "https://www.parcio.ovgu.de"
HOSTS = ("www.parcio.ovgu.de", "parcio.ovgu.de")
START = "/Homepage-p-1.html"


# The pagination of the news list, which is no page of its own.
PAGINATION = re.compile(r"-pos-\d+\.html$")


def page_type(path):
    """Generated pages (lists, publications, the sitemap) and people pages get other types than
    text pages, as on the real site. The real types are unknown, these are types of the demo site."""
    if path == "/People.html":
        return "testimonials/list"
    if path.startswith("/People/"):
        return "testimonials/entry"
    if path == "/News.html":
        return "blog/list"
    if path.startswith("/News/"):
        return "blog/entry"
    if path.startswith("/Publications"):
        return "copyright"
    if path == "/Sitemap.html":
        return "search/content"
    return "page"


class Cms:
    """The few calls of the EgoCMS REST API the seeding needs."""

    def __init__(self, url, site, language, user_id, token):
        self.base = f"{url}/rest/"
        self.site_base = f"{self.base}{site}/{language}/"
        context = ssl.create_default_context()
        if "localhost" in url:
            context.check_hostname = False
            context.verify_mode = ssl.CERT_NONE
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()),
            urllib.request.HTTPSHandler(context=context),
        )
        query = urllib.parse.urlencode({"user_id": user_id, "token": token})
        self.request("PUT", f"{self.base}startSession?{query}")

    def request(self, method, url, body=None, content_type="application/json"):
        data = json.dumps(body).encode() if content_type == "application/json" and body is not None else body
        request = urllib.request.Request(url, data=data, method=method)
        if data is not None:
            request.add_header("Content-Type", content_type)
        with self.opener.open(request) as response:
            text = response.read().decode()
        result = json.loads(text) if text.strip() else None
        if isinstance(result, dict) and "error" in result:
            raise RuntimeError(f"{url}: {result['error']}")
        return result

    def children(self, page_id):
        return self.request("GET", f"{self.site_base}{page_id}/getChildren") or []

    def delete(self, page_id):
        self.request("DELETE", f"{self.site_base}{page_id}/delete")

    def new_child(self, parent_id, name, title, page_type):
        field = {"name": name, "title": title, "type": page_type, "inactive": 0, "nav_hide": 0}
        page = self.request("POST", f"{self.site_base}{parent_id}/newChild", {"field": field, "extra": {}})
        return page["field"]["id"]

    def update(self, page_id, name, title, content):
        extra = {"_blocks": {"center": ["text"]}, "_contents": {"center": [{"content1": content}]}}
        body = {"param": {"field": {"name": name, "title": title}, "extra": extra}}
        self.request("PUT", f"{self.site_base}{page_id}/update", body)

    def attach(self, page_id, name, data):
        # Mediapool::put($source, $name): `@data` holds the arguments, "@file" stands for the upload.
        boundary = uuid.uuid4().hex
        body = (
            f'--{boundary}\r\nContent-Disposition: form-data; name="@data"\r\n\r\n'
            f"{json.dumps(['@file', name])}\r\n"
            f'--{boundary}\r\nContent-Disposition: form-data; name="upload"; filename="placeholder"\r\n'
            f"Content-Type: application/octet-stream\r\n\r\n"
        ).encode() + data + f"\r\n--{boundary}--\r\n".encode()
        self.request("POST", f"{self.site_base}{page_id}/pool/put", body, f"multipart/form-data; boundary={boundary}")


def placeholder(name):
    """A small file of the right kind: an image for images, a PDF for PDFs, text otherwise."""
    extension = name.rsplit(".", 1)[-1].lower()
    if extension in ("png", "jpg", "jpeg", "gif"):
        width = height = 64
        rows = b"".join(b"\x00" + bytes([200, 220, 240]) * width for _ in range(height))

        def chunk(kind, data):
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

        header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
        return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")
    if extension == "pdf":
        return f"%PDF-1.4\n% Placeholder for {name}\n%%EOF\n".encode()
    return f"Placeholder for {name}\n".encode()


def depth(path, pages):
    """The number of ancestors of a page, to create parents before their children."""
    count = 0
    while (path := pages[path]["parent"]) is not None and path in pages:
        count += 1
    return count


class LinkRewriter:
    """Turns the links of the website back into the editor's format, collecting the attachments."""

    def __init__(self, ids, site, language):
        self.ids, self.site, self.language = ids, site, language
        # Attachments live at `<page URL without .html>/_/<name>`.
        self.bases = {path.removesuffix(".html"): path for path in ids}
        self.bases[""] = self.bases[START.removesuffix(".html")] = "/"
        self.attachments = set()

    def rewrite(self, content):
        return re.sub(r'(href|src)="([^"]*)"', lambda m: f'{m[1]}="{self.url(m[2])}"', content)

    def url(self, value):
        url = urllib.parse.urlparse(urllib.parse.urljoin(BASE + "/", html.unescape(value)))
        if url.scheme not in ("http", "https") or url.netloc not in HOSTS:
            return value
        anchor = f"#{url.fragment}" if url.fragment else ""
        if "/_/" in url.path:
            base, name = url.path.split("/_/", 1)
            owner = self.bases.get(base)
            if owner is not None:
                name = urllib.parse.unquote(name)
                self.attachments.add((owner, name))
                query = urllib.parse.urlencode({"pool": name, "site": self.site, "lang": self.language, "id": self.ids[owner]})
                return html.escape(f"index.php?{query}")
        path = "/" if url.path in ("/", START) else url.path
        if path in self.ids:
            return html.escape(f"index.php?site={self.site}&id={self.ids[path]}&lang={self.language}{anchor}")
        # A page that isn't part of the mirror, or a file of the media site: keep the real one.
        return html.escape(url._replace(netloc="www.parcio.ovgu.de", scheme="https").geturl())


def main():
    arguments = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    arguments.add_argument("--url", default="https://localhost")
    arguments.add_argument("--site", default="materialkit")
    arguments.add_argument("--language", default="de")
    arguments.add_argument("--parent", type=int, default=1, help="id of the page the mirror is created below")
    arguments.add_argument("--name", default="ParCIO", help="name of the mirror's home page")
    options = arguments.parse_args()

    pages = json.loads((HERE / "snapshot.json").read_text())["pages"]
    pages = {path: page for path, page in pages.items() if not PAGINATION.search(path)}
    cms = Cms(options.url, options.site, options.language, os.environ["EGOCMS_USER_ID"], os.environ["EGOCMS_TOKEN"])

    # 1. Replace an existing mirror.
    for page in cms.children(options.parent):
        if page["field"]["name"] == options.name:
            print(f"Deleting the old mirror (id {page['field']['id']}).")
            cms.delete(page["field"]["id"])

    # 2. Create all pages, parents first, so every page has an id.
    ids = {"/": cms.new_child(options.parent, options.name, pages["/"]["title"], "page")}
    for path in sorted(pages, key=lambda path: depth(path, pages)):
        page = pages[path]
        if path == "/":
            continue
        if page["parent"] not in ids:
            print(f"Skipping {path}: its parent {page['parent']} is not part of the snapshot.")
            continue
        ids[path] = cms.new_child(ids[page["parent"]], page["name"], page["title"] or page["name"], page_type(path))

    # 3. Write the contents, with links in the editor's format, and attach the linked files.
    links = LinkRewriter(ids, options.site, options.language)
    for path, page_id in ids.items():
        page = pages[path]
        name = options.name if path == "/" else page["name"]
        cms.update(page_id, name, page["title"] or name, links.rewrite(page["html"]))
    for owner, name in sorted(links.attachments):
        cms.attach(ids[owner], name, placeholder(name))

    print(f"Created {len(ids)} pages with {len(links.attachments)} placeholder files below {options.name} (id {ids['/']}).")
    config = HERE / "mirror.toml"
    config.write_text(
        "# Written by seed.py: runs the updater against the local parcio mirror.\n"
        f'url = "{options.url}"\nsite = "{options.site}"\nlanguage = "{options.language}"\n'
        f"accept_invalid_certs = true\nhome_page_id = {ids['/']}\n"
        f'pages_dir = "{HERE / "pages"}"\n'
    )
    print(f"Wrote {config}: cargo run -- --config-path {config} download {HERE / 'pages'}")


if __name__ == "__main__":
    main()
