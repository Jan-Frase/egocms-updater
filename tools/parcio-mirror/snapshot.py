#!/usr/bin/env python3
"""Takes a snapshot of the public parcio website, so `seed.py` can rebuild it in a local EgoCMS.

For every page reachable from the start page it saves the name (from the breadcrumb), the title,
the parent and the HTML of the text area into `snapshot.json`. Requests are spaced out, and the
snapshot is saved after every page, so an interrupted run continues where it stopped.

Only uses the Python standard library: `python3 snapshot.py`
"""

import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from html.parser import HTMLParser
from pathlib import Path

BASE = "https://www.parcio.ovgu.de"
START = "/Homepage-p-1.html"
# Seconds between two requests. The site answers with 429 when asked too quickly.
DELAY = 2.5
# Pages that are no content pages: the feedback form and the pagination of the news list.
EXCLUDE = re.compile(r"^/Feedback-|-pos-\d+\.html$")
SNAPSHOT = Path(__file__).with_name("snapshot.json")
HEADERS = {"User-Agent": "egocms-updater snapshot (local testing)"}


class PageParser(HTMLParser):
    """Collects the breadcrumb, the title, the text area and all links of a page."""

    def __init__(self, html):
        super().__init__(convert_charrefs=True)
        self.html = html
        self.line_starts = [0]
        for line in html.splitlines(keepends=True):
            self.line_starts.append(self.line_starts[-1] + len(line))
        self.breadcrumb = []  # (href, name)
        self.title = ""
        self.links = []
        self.in_title = False
        # Tracks <div>/<section> nesting to find the direct children of section#content.
        self.level = 0
        self.content_level = None
        self.child = None
        self.children = []  # dicts with inner start/end offsets

    def position(self):
        line, column = self.getpos()
        return self.line_starts[line - 1] + column

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag in ("div", "section"):
            self.level += 1
            if tag == "section" and attrs.get("id") == "content":
                self.content_level = self.level
            elif self.content_level is not None and self.level == self.content_level + 1:
                start = self.position() + len(self.get_starttag_text())
                self.child = {"start": start, "id": attrs.get("id", ""), "has_title": False}
        if tag == "h1" and attrs.get("id") == "page-title":
            self.in_title = True
            if self.child is not None:
                self.child["has_title"] = True
        if tag == "a" and attrs.get("href"):
            self.links.append(attrs["href"])
            if "breadcrumb_link" in attrs.get("class", ""):
                self.breadcrumb.append((attrs["href"], attrs.get("alt", "")))

    def handle_endtag(self, tag):
        if tag == "h1":
            self.in_title = False
        if tag in ("div", "section"):
            if self.child is not None and self.level == self.content_level + 1:
                self.child["end"] = self.position()
                self.children.append(self.child)
                self.child = None
            if self.level == self.content_level:
                self.content_level = None
            self.level -= 1

    def handle_data(self, data):
        if self.in_title:
            self.title += data

    def content_html(self):
        """The text area: everything in section#content after the block with the title."""
        titles = [i for i, child in enumerate(self.children) if child["has_title"]]
        start = titles[0] + 1 if titles else 0
        parts = [
            self.html[child["start"] : child["end"]].strip()
            for child in self.children[start:]
            if not child["id"].startswith("readspeaker")
        ]
        return "\n".join(part for part in parts if part)


def normalize(href, current):
    """The site path of a link to a page of the site, or None for anything else."""
    url = urllib.parse.urlparse(urllib.parse.urljoin(BASE + current, href))
    if url.netloc not in ("www.parcio.ovgu.de", "parcio.ovgu.de"):
        return None
    # Links are written both as `ü` and as `%C3%BC`, so paths are kept decoded (but `+` stays `+`).
    path = urllib.parse.unquote(url.path)
    if path in ("/", START):
        return "/"
    if not path.endswith(".html") or "/_/" in path or EXCLUDE.search(path):
        return None
    return path


def fetch(path):
    """Fetches a page, waiting longer and longer when the site asks to slow down."""
    for pause in (30, 60, 120, None):
        try:
            url = BASE + urllib.parse.quote(START if path == "/" else path, safe="/+")
            request = urllib.request.Request(url, headers=HEADERS)
            with urllib.request.urlopen(request, timeout=60) as response:
                return response.read().decode("utf-8")
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            if error.code != 429 or pause is None:
                raise
            print(f"  The site asks to slow down, waiting {pause}s.")
            time.sleep(pause)
    return None


def main():
    snapshot = json.loads(SNAPSHOT.read_text()) if SNAPSHOT.exists() else {"pages": {}, "queue": ["/"]}
    # Snapshots of older versions contain encoded paths.
    decode = urllib.parse.unquote
    snapshot["pages"] = {
        decode(path): {**page, "parent": page["parent"] and decode(page["parent"])}
        for path, page in snapshot["pages"].items()
    }
    snapshot["queue"] = list(dict.fromkeys(decode(path) for path in snapshot["queue"]))
    pages, queue = snapshot["pages"], snapshot["queue"]
    while queue:
        path = queue.pop(0)
        if path in pages:
            continue
        print(f"[{len(pages) + 1}] {urllib.parse.unquote_plus(path)}")
        html = fetch(path)
        time.sleep(DELAY)
        if html is None:
            continue

        parser = PageParser(html)
        parser.feed(html)
        crumbs = [(normalize(href, path), name) for href, name in parser.breadcrumb]
        if path != "/" and len(crumbs) < 2:
            print("  No breadcrumb, skipped.")
            continue
        pages[path] = {
            "name": crumbs[-1][1] if path != "/" else "Homepage",
            "title": " ".join(parser.title.split()),
            "parent": crumbs[-2][0] if path != "/" else None,
            "html": parser.content_html(),
        }
        for href in parser.links:
            target = normalize(href, path)
            if target and target not in pages and target not in queue:
                queue.append(target)
        SNAPSHOT.write_text(json.dumps(snapshot, indent=1, ensure_ascii=False))

    print(f"Done: {len(pages)} pages in {SNAPSHOT}.")


if __name__ == "__main__":
    sys.exit(main())
