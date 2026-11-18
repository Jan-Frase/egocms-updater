#!/usr/bin/env python3
"""Wipes sites of the local EgoCMS test instance: deletes every page below the root page, empties
the trash, and removes the root page's attachments and updater marker. Only the root page remains.

Only uses the Python standard library:
    export EGOCMS_USER_ID=… EGOCMS_TOKEN=…
    python3 wipe.py                        # wipes `materialkit`
    python3 wipe.py materialkit test       # wipes several sites
    python3 wipe.py --dry-run              # only lists what would be deleted
"""

import argparse
import http.cookiejar
import json
import os
import ssl
import sys
import urllib.error
import urllib.parse
import urllib.request


class Cms:
    """The few calls of the EgoCMS REST API the wipe needs."""

    def __init__(self, url, user_id, token):
        self.base = f"{url}/rest/"
        context = ssl.create_default_context()
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()),
            urllib.request.HTTPSHandler(context=context),
        )
        query = urllib.parse.urlencode({"user_id": user_id, "token": token})
        self.request("PUT", f"startSession?{query}")

    def request(self, method, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        request = urllib.request.Request(self.base + path, data=data, method=method)
        if data is not None:
            request.add_header("Content-Type", "application/json")
        # Deleting a large tree can take minutes.
        with self.opener.open(request, timeout=600) as response:
            text = response.read().decode()
        result = json.loads(text) if text.strip() else None
        if isinstance(result, dict) and "error" in result:
            raise RuntimeError(f"{path}: {result['error']}")
        return result


def delete_tree(cms, prefix, page_id, deleted):
    """Deletes a page after all pages below it, collecting the ids of deleted pages in `deleted`.
    A page can be linked below several parents, so it may come up more than once."""
    if page_id in deleted:
        return
    for child in cms.request("GET", f"{prefix}{page_id}/getChildren") or []:
        delete_tree(cms, prefix, child["field"]["id"], deleted)
    try:
        cms.request("DELETE", f"{prefix}{page_id}/delete")
    except urllib.error.HTTPError as error:
        # Every failed delete is a 400. It's fine if the page is gone anyway.
        if error.code != 400 or cms.request("GET", f"{prefix}getPage?id={page_id}") is not None:
            raise
    deleted.add(page_id)


def empty_trash(cms, prefix, attempts=10):
    """Empties the trash. A large trash takes several attempts, as PHP stops each request after 30
    seconds, but every attempt destroys a part of it."""
    for _ in range(attempts):
        try:
            cms.request("POST", prefix + "clearTrashcan")
        except urllib.error.HTTPError as error:
            if error.code != 500:
                raise
        if not cms.request("GET", prefix + "hasDeleted"):
            return
        print("  The trash is not empty yet, trying again.")
    sys.exit(f"The trash of {prefix} could not be emptied, run the script again.")


def wipe(cms, site, language, dry_run):
    prefix = f"{site}/{language}/"
    root = cms.request("GET", prefix + "getRoot")
    root_id = root["field"]["id"]
    children = cms.request("GET", f"{prefix}{root_id}/getChildren") or []
    pool = [file["name"] for file in cms.request("GET", f"{prefix}{root_id}/pool/list") or []]
    print(f"{site}: {len(children)} pages below the root page {root['field']['name']} (id {root_id}), "
          f"{len(pool)} files attached to it.")
    for child in children:
        print(f"  {child['field']['name']} (id {child['field']['id']})")
    if dry_run:
        return

    # PHP stops every request after 30 seconds, and deleting a large tree at once takes longer.
    # Deleting the deepest pages first keeps each request small.
    deleted = set()
    for child in children:
        delete_tree(cms, prefix, child["field"]["id"], deleted)
    print(f"{site}: deleted {len(deleted)} pages, emptying the trash.")
    empty_trash(cms, prefix)
    for name in pool:
        cms.request("DELETE", f"{prefix}{root_id}/pool/delete?{urllib.parse.urlencode({'name': name})}")

    # Without the marker, the updater treats the root page like a page made in the editor again.
    extra = root.get("extra") or {}
    if isinstance(extra, dict) and extra.pop("_sync", None) is not None:
        body = {"param": {"field": {"name": root["field"]["name"], "title": root["field"]["title"]}, "extra": extra}}
        cms.request("PUT", f"{prefix}{root_id}/update", body)
    print(f"{site}: wiped.")


def main():
    arguments = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    arguments.add_argument("sites", nargs="*", default=["materialkit"])
    arguments.add_argument("--url", default="https://localhost")
    arguments.add_argument("--language", default="de")
    arguments.add_argument("--dry-run", action="store_true", help="only list what would be deleted")
    arguments.add_argument("--yes", action="store_true", help="don't ask before deleting")
    options = arguments.parse_args()

    host = urllib.parse.urlparse(options.url).hostname
    if host not in ("localhost", "127.0.0.1"):
        sys.exit(f"Refusing to wipe {options.url}: this script is only meant for the local test instance.")

    credentials = [os.environ.get(name) for name in ("EGOCMS_USER_ID", "EGOCMS_TOKEN")]
    if not all(credentials):
        sys.exit("Set the environment variables EGOCMS_USER_ID and EGOCMS_TOKEN.")
    cms = Cms(options.url, *credentials)
    if not options.dry_run and not options.yes:
        answer = input(f"Delete all pages of {', '.join(options.sites)} on {options.url} except the root page? [y/N] ")
        if answer.strip().lower() != "y":
            sys.exit("Nothing deleted.")
    for site in options.sites:
        wipe(cms, site, options.language, options.dry_run)


if __name__ == "__main__":
    main()
