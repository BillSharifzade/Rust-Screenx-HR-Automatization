#!/usr/bin/env python3
"""Run every image in assets/ through the interview-form OCR API and dump the results.

The API only fetches images over http(s), so this starts a throwaway nginx
container on the backend's own docker network and hands the backend
`http://<sidecar>/<file>` URLs. Nothing is exposed outside that network.

    python3 tools/ocr_assets_test.py

Stdlib only. Needs `docker` on PATH and a running `recruitment-backend` container.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

IMAGE_SUFFIXES = (".jpg", ".jpeg", ".png", ".webp")
SIDECAR = "ocr-assets-test"
SIDECAR_IMAGE = "nginx:alpine"


# --------------------------------------------------------------------------- http


def request(
    method: str,
    url: str,
    payload: dict | None = None,
    timeout: int = 120,
    retries: int = 3,
):
    """Returns (status, body). status 0 means the connection itself failed."""
    data = json.dumps(payload).encode() if payload is not None else None
    for attempt in range(retries + 1):
        req = urllib.request.Request(url, data=data, method=method)
        if data:
            req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(req, timeout=timeout) as res:
                body = res.read().decode("utf-8", "replace")
                return res.status, (json.loads(body) if body.strip() else None)
        except urllib.error.HTTPError as e:
            body = e.read().decode("utf-8", "replace")
            try:
                return e.code, json.loads(body)
            except json.JSONDecodeError:
                return e.code, {"error": body[:500]}
        except OSError as e:
            # Includes URLError and the ConnectionResetError docker-proxy throws
            # while the container is up but the app is not yet listening.
            if attempt >= retries:
                return 0, {"error": f"{type(e).__name__}: {e}"}
            time.sleep(2)
    return 0, {"error": "unreachable"}


def fetch_bytes(url: str, timeout: int = 120) -> bytes | None:
    try:
        with urllib.request.urlopen(urllib.request.Request(url), timeout=timeout) as res:
            return res.read()
    except (urllib.error.HTTPError, OSError) as e:
        print(f"  {url} -> {type(e).__name__}: {e}", file=sys.stderr)
        return None


def wait_for_backend(base_url: str, container: str, seconds: int) -> bool:
    deadline = time.time() + seconds
    announced = False
    while True:
        status, body = request("GET", f"{base_url}/health", timeout=10, retries=0)
        if status == 200:
            if announced:
                print()
            return True
        if time.time() > deadline:
            print()
            print(f"backend never answered on {base_url}/health ({status}: {body})", file=sys.stderr)
            if shutil.which("docker"):
                logs = docker("logs", "--tail", "20", container, check=False)
                if logs:
                    print(f"--- last lines of `docker logs {container}` ---", file=sys.stderr)
                    print(logs, file=sys.stderr)
            return False
        if not announced:
            print(f"waiting for {base_url}/health", end="", flush=True)
            announced = True
        print(".", end="", flush=True)
        time.sleep(2)


# ------------------------------------------------------------------------- docker


def docker(*args: str, check: bool = True) -> str:
    proc = subprocess.run(
        ["docker", *args], capture_output=True, text=True, check=False
    )
    if check and proc.returncode != 0:
        raise RuntimeError(
            f"docker {' '.join(args)} failed ({proc.returncode}): {proc.stderr.strip()}"
        )
    return proc.stdout.strip()


def backend_network(container: str) -> str:
    nets = docker(
        "inspect",
        "-f",
        "{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}",
        container,
    ).split()
    if not nets:
        raise RuntimeError(f"container {container} is not attached to any network")
    # A compose service normally sits on exactly one user-defined network.
    return nets[0]


def start_sidecar(assets_dir: str, network: str) -> None:
    docker("rm", "-f", SIDECAR, check=False)
    docker(
        "run",
        "-d",
        "--name",
        SIDECAR,
        "--network",
        network,
        "-v",
        f"{assets_dir}:/usr/share/nginx/html:ro",
        SIDECAR_IMAGE,
    )
    for _ in range(30):
        state = docker(
            "inspect", "-f", "{{.State.Running}}", SIDECAR, check=False
        )
        if state == "true":
            time.sleep(1)
            return
        time.sleep(1)
    raise RuntimeError(f"{SIDECAR} did not come up; see `docker logs {SIDECAR}`")


# ------------------------------------------------------------------------ render


def val(v) -> str:
    if v is None or v == "" or v == []:
        return "—"
    if isinstance(v, list):
        return ", ".join(str(x) for x in v)
    return str(v)


def render_markdown(rec: dict, filename: str) -> str:
    f = rec.get("fields") or {}
    conf = rec.get("field_confidence") or {}
    out: list[str] = []
    add = out.append

    add(f"# {filename}")
    add("")
    add(f"- **form_type**: `{rec.get('form_type')}`")
    overall = rec.get("overall_confidence")
    add(f"- **overall_confidence**: {overall if overall is not None else '—'}")
    add(f"- **needs_review**: {rec.get('needs_review')}")
    add(f"- **recognition_id**: `{rec.get('id')}`")
    add("")

    add("## Header")
    add("")
    add("| field | value | conf |")
    add("| --- | --- | --- |")
    for key in (
        "candidate_name",
        "candidate_age",
        "position_discussed",
        "interview_date",
        "scheduled_start_time",
        "actual_arrival_time",
        "interview_from",
        "interview_to",
        "interviewers",
        "interviewer_position",
        "department",
        "division",
    ):
        c = conf.get(key)
        add(f"| {key} | {val(f.get(key))} | {c if c is not None else '—'} |")
    add("")

    params = f.get("parameters") or []
    if params:
        add("## Parameters")
        add("")
        add("| key | label | value |")
        add("| --- | --- | --- |")
        for p in params:
            add(f"| {val(p.get('key'))} | {val(p.get('label'))} | {val(p.get('value'))} |")
        add("")

    for title, key in (("Strengths", "strengths"), ("Growth areas", "growth_areas")):
        rows = f.get(key) or []
        if rows:
            add(f"## {title}")
            add("")
            add("| # | prof/soft skills | personal qualities |")
            add("| --- | --- | --- |")
            for r in rows:
                add(
                    f"| {val(r.get('index'))} | {val(r.get('prof_soft_skills'))} "
                    f"| {val(r.get('personal_qualities'))} |"
                )
            add("")

    for title, key in (
        ("Comments", "comments"),
        ("Conclusions", "conclusions"),
        ("HR department recommendation", "hr_department_recommendation"),
        ("Test results (BUD)", "test_results_bud"),
        ("Test results (FED)", "test_results_fed"),
    ):
        if f.get(key):
            add(f"## {title}")
            add("")
            add(str(f[key]))
            add("")

    for title, key in (
        ("Requester decision", "requester_decision"),
        ("HR decision", "hr_decision"),
    ):
        block = f.get(key)
        if block and any(block.get(k) for k in ("full_name", "position", "comment")):
            add(f"## {title}")
            add("")
            add(f"- full_name: {val(block.get('full_name'))}")
            add(f"- position: {val(block.get('position'))}")
            add(f"- comment: {val(block.get('comment'))}")
            add("")

    extra = f.get("extra_notes") or []
    if extra:
        add("## Extra notes")
        add("")
        for note in extra:
            add(f"- {note}")
        add("")

    low = rec.get("low_confidence_fields") or []
    warnings = rec.get("warnings") or []
    if low or warnings:
        add("## Review flags")
        add("")
        for w in warnings:
            add(f"- ⚠️  {w}")
        for path in low:
            add(f"- low confidence: `{path}` ({conf.get(path)})")
        add("")

    return "\n".join(out)


# --------------------------------------------------------------------------- main


def main() -> int:
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", default="http://localhost:8888")
    ap.add_argument("--assets", default=os.path.join(here, "recruitment-backend", "assets"))
    ap.add_argument("--out", default=os.path.join(here, "ocr-out"))
    ap.add_argument("--container", default="recruitment-backend")
    ap.add_argument("--batch", type=int, default=10, help="API cap is 10 per job")
    ap.add_argument("--timeout", type=int, default=2400, help="seconds to wait per job")
    ap.add_argument(
        "--wait", type=int, default=120, help="seconds to wait for the backend to start listening"
    )
    ap.add_argument("--form-type", default=None, choices=[None, "interview_1", "interview_2"])
    ap.add_argument("--keep-sidecar", action="store_true")
    ap.add_argument(
        "--skip-existing",
        action="store_true",
        help="don't re-send images that already have a result in --out",
    )
    ap.add_argument("--only", nargs="+", metavar="FILE", help="only these filenames")
    ap.add_argument(
        "--pdf",
        action="store_true",
        help="also download each form as a PDF laid out like the paper sheet",
    )
    ap.add_argument(
        "--url-prefix",
        default=None,
        help="serve the images yourself and skip the nginx sidecar, e.g. https://host/assets/",
    )
    args = ap.parse_args()

    assets_dir = os.path.abspath(args.assets)
    images = sorted(
        f
        for f in os.listdir(assets_dir)
        if f.lower().endswith(IMAGE_SUFFIXES) and os.path.isfile(os.path.join(assets_dir, f))
    )
    if not images:
        print(f"no images in {assets_dir}", file=sys.stderr)
        return 1

    if args.only:
        wanted = set(args.only)
        unknown = wanted - set(images)
        if unknown:
            print(f"not in {assets_dir}: {', '.join(sorted(unknown))}", file=sys.stderr)
            return 1
        images = [f for f in images if f in wanted]

    # Results already on disk, so a re-run after a rate limit only pays for the
    # pages that are actually missing.
    preloaded: list[tuple[str, dict]] = []
    if args.skip_existing:
        keep = []
        for name in images:
            path = os.path.join(args.out, os.path.splitext(name)[0] + ".json")
            if os.path.exists(path):
                with open(path) as fh:
                    preloaded.append((name, json.load(fh)))
            else:
                keep.append(name)
        if preloaded:
            print(f"skipping {len(preloaded)} image(s) already in {args.out}")
        images = keep
        if not images:
            print("nothing left to recognize")
            if not args.pdf:
                return 0

    if not wait_for_backend(args.base_url, args.container, args.wait):
        return 1
    status, schema = request("GET", f"{args.base_url}/api/onef/interview-forms/schema")
    if status != 200:
        print(
            f"{args.base_url} has no /api/onef/interview-forms/schema ({status}) — "
            "the running container predates the OCR feature; `docker compose up -d backend`",
            file=sys.stderr,
        )
        return 1

    # Only needed to hand the backend fetchable URLs; a PDF-only run has nothing to serve.
    sidecar_started = False
    prefix = ""
    if images:
        if args.url_prefix:
            prefix = args.url_prefix if args.url_prefix.endswith("/") else args.url_prefix + "/"
        else:
            if not shutil.which("docker"):
                print("docker not on PATH; pass --url-prefix instead", file=sys.stderr)
                return 1
            network = backend_network(args.container)
            print(f"serving {assets_dir} on docker network {network} as {SIDECAR}")
            start_sidecar(assets_dir, network)
            sidecar_started = True
            prefix = f"http://{SIDECAR}/"

    os.makedirs(args.out, exist_ok=True)
    started = time.time()
    recognitions: list[tuple[str, dict]] = list(preloaded)
    jobs: list[dict] = []

    try:
        batches = [images[i : i + args.batch] for i in range(0, len(images), args.batch)]
        pending = []
        for batch in batches:
            payload = {
                "image_urls": [prefix + urllib.parse.quote(name) for name in batch],
                "external_ref": "assets-ocr-test",
            }
            if args.form_type:
                payload["form_type"] = args.form_type
            status, body = request(
                "POST", f"{args.base_url}/api/onef/interview-forms/recognize", payload
            )
            if status != 202:
                print(f"recognize failed ({status}): {body}", file=sys.stderr)
                return 1
            job_id = body["job_id"]
            print(f"job {job_id} accepted — {len(batch)} images")
            pending.append((job_id, batch))

        deadline = time.time() + args.timeout
        for job_id, batch in pending:
            last = None
            while True:
                status, body = request(
                    "GET", f"{args.base_url}/api/onef/interview-forms/jobs/{job_id}"
                )
                if status != 200:
                    # Don't throw away a job that is still running over one bad poll.
                    print(f"  poll failed ({status}): {body} — retrying", file=sys.stderr)
                    if time.time() > deadline:
                        return 1
                    time.sleep(5)
                    continue
                state = body.get("status")
                pages = len(body.get("forms") or [])
                if (state, pages) != last:
                    print(f"  job {job_id}: {state} — {pages}/{len(batch)} pages")
                    last = (state, pages)
                if state in ("completed", "partial", "failed"):
                    jobs.append(body)
                    break
                if time.time() > deadline:
                    print(f"timed out waiting on job {job_id}", file=sys.stderr)
                    jobs.append(body)
                    break
                time.sleep(5)

            with open(os.path.join(args.out, f"job-{job_id}.json"), "w") as fh:
                json.dump(jobs[-1], fh, ensure_ascii=False, indent=2)
            if jobs[-1].get("error"):
                print(f"  job {job_id} error: {jobs[-1]['error']}")

            for rec in jobs[-1].get("forms") or []:
                name = urllib.parse.unquote(rec["source_url"].rsplit("/", 1)[-1])
                stem = os.path.splitext(name)[0]
                with open(os.path.join(args.out, f"{stem}.json"), "w") as fh:
                    json.dump(rec, fh, ensure_ascii=False, indent=2)
                with open(os.path.join(args.out, f"{stem}.md"), "w") as fh:
                    fh.write(render_markdown(rec, name))
                recognitions.append((name, rec))
    finally:
        if sidecar_started and not args.keep_sidecar:
            docker("rm", "-f", SIDECAR, check=False)

    if args.pdf and recognitions:
        print(f"downloading {len(recognitions)} PDF(s)")
        for name, rec in recognitions:
            blob = fetch_bytes(
                f"{args.base_url}/api/onef/interview-forms/results/{rec['id']}/pdf"
            )
            if blob:
                with open(os.path.join(args.out, os.path.splitext(name)[0] + ".pdf"), "wb") as fh:
                    fh.write(blob)

    elapsed = int(time.time() - started)
    lines = [
        "# assets/ OCR test run",
        "",
        f"- images: {len(images) + len(preloaded)}",
        f"- recognized: {len(recognitions)}",
        f"- jobs: " + ", ".join(f"{j['id']} ({j['status']})" for j in jobs),
        f"- wall clock: {elapsed}s",
        "",
        "| file | form_type | candidate | date | conf | review | warnings |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for name, rec in sorted(recognitions):
        f = rec.get("fields") or {}
        conf = rec.get("overall_confidence")
        lines.append(
            f"| {name} | {rec.get('form_type')} | {val(f.get('candidate_name'))} "
            f"| {val(f.get('interview_date'))} | {conf if conf is not None else '—'} "
            f"| {'yes' if rec.get('needs_review') else 'no'} "
            f"| {len(rec.get('warnings') or [])} |"
        )
    failed = [n for n in images if n not in {r[0] for r in recognitions}]
    if failed:
        lines += ["", "## Not recognized", ""] + [f"- {n}" for n in failed]

    summary = "\n".join(lines)
    with open(os.path.join(args.out, "summary.md"), "w") as fh:
        fh.write(summary + "\n")
    print()
    print(summary)
    print()
    print(f"wrote {args.out}/")
    return 0 if len(recognitions) == len(images) + len(preloaded) else 2


if __name__ == "__main__":
    sys.exit(main())
