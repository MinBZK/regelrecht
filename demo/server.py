"""Minimal backend for the graph frontend.

Serves two endpoints that the Svelte graph expects:
  GET /laws/list          → JSON array of YAML file paths
  GET /law/{path}         → raw YAML content
  GET /laws/demo-selection → empty array (select all)

Also proxies to the Vite dev server for the frontend.
"""

import glob
import os
from pathlib import Path

import uvicorn
import yaml
from fastapi import FastAPI
from fastapi.responses import JSONResponse, PlainTextResponse

CORPUS_DIR = Path(__file__).resolve().parent.parent / "corpus" / "regulation" / "nl"

app = FastAPI()


def discover_laws() -> list[str]:
    """Find all YAML files in the corpus, return relative paths."""
    paths = []
    for yamlfile in sorted(CORPUS_DIR.rglob("*.yaml")):
        if "/scenarios/" in str(yamlfile):
            continue
        rel = str(yamlfile.relative_to(CORPUS_DIR))
        paths.append(rel)
    return paths


@app.get("/laws/list")
async def list_laws():
    return JSONResponse(content=discover_laws())


@app.get("/laws/demo-selection")
async def demo_selection():
    return JSONResponse(content=[])


@app.get("/law/{path:path}")
async def get_law(path: str):
    full_path = CORPUS_DIR / path
    if not full_path.exists() or not str(full_path.resolve()).startswith(str(CORPUS_DIR)):
        return PlainTextResponse("Not found", status_code=404)
    return PlainTextResponse(full_path.read_text())


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8000)
