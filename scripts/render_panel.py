"""Screenshot a panel page headless (Microsoft Edge) with a given state, for checking screens.

  uv run scripts/render_panel.py out.png                       demo client screen
  uv run scripts/render_panel.py out.png --demo status
  uv run scripts/render_panel.py out.png --state state.json --click ".rec" --height 1080
  uv run scripts/render_panel.py out.png --state state.json --explore report.json
  uv run scripts/render_panel.py out.png --page game.html --demo 1 --width 827 --scale 1.333 --mode ingame
  uv run scripts/render_panel.py out.png --page game.html --state game.json --mode ingame --hover ".ig-opt .ig-vs b[title]"
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

PANEL = Path(__file__).resolve().parent.parent / "panel"
EDGE_PATHS = [
    Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Microsoft/Edge/Application/msedge.exe",
    Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "Microsoft/Edge/Application/msedge.exe",
]


# Hover: fire the panel's own mouseover (in game it draws #tip from the title), fall back to drawing
# the same #tip elsewhere (native tooltips aren't in screenshots), then draw a mouse cursor on top.
# Redone on resize: headless Edge only reaches --window-size when it takes the screenshot, and the
# client page re-zooms to its window then.
HOVER_JS = """function(sel){
  function hover(){
    ["tip","hovercur"].forEach(function(id){var e=document.getElementById(id);if(e)e.remove()});
    var t=document.querySelector(sel);if(!t)return;
    var r=t.getBoundingClientRect(),x=r.left+r.width*0.6,y=r.top+r.height*0.6;
    t.dispatchEvent(new MouseEvent("mouseover",{bubbles:true,clientX:x,clientY:y}));
    var tipped=t.closest("[title],[data-tip]");
    if(!document.getElementById("tip")&&tipped&&(tipped.title||tipped.dataset.tip)){
      var tip=document.createElement("div");tip.id="tip";tip.textContent=tipped.title||tipped.dataset.tip;document.body.appendChild(tip);
      var W=document.body.clientWidth,H=document.body.clientHeight,b=tipped.getBoundingClientRect();
      tip.style.maxWidth=(W-4)+"px";
      tip.style.left=Math.max(2,Math.min(W-tip.offsetWidth-2,b.left))+"px";
      tip.style.top=(b.bottom+tip.offsetHeight+4<H?b.bottom+3:b.top-tip.offsetHeight-3)+"px";
    }
    var c=document.createElement("div");c.id="hovercur";
    c.style.cssText="position:fixed;z-index:99;pointer-events:none;left:"+x+"px;top:"+y+"px;width:16px;height:24px";
    c.innerHTML='<svg width="16" height="24" viewBox="0 0 16 24"><path d="M1 1v19l5-5 3.5 7.5 3-1.4L9 13.6h6.5z" fill="#fff" stroke="#000" stroke-width="1.2" stroke-linejoin="round"/></svg>';
    document.body.appendChild(c);
  }
  hover();
  window.addEventListener("resize",function(){requestAnimationFrame(hover)});
}"""


def render(out: Path, state: dict | None = None, demo: str | None = None, click: list[str] | None = None,
           width: int = 300, height: int = 900, scale: float = 1.0, mode: str = "",
           hover: str | None = None, page: str = "index.html", explore: dict | None = None) -> Path:
    """Screenshot `panel/<page>` after pushing `state` (and `explore`, the explorer's data) to it."""
    edge = next((p for p in EDGE_PATHS if p.exists()), None)
    if not edge:
        raise SystemExit("Microsoft Edge not found")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / "panel"
        shutil.copytree(PANEL, root)
        script = [f"setScale({scale})", f"setMode({json.dumps(mode)})"]
        if state is not None:
            script.append(f"updateState({json.dumps(state)})")
        if explore is not None:
            script.append(f"renderExplore({json.dumps(explore)})")
        for sel in click or []:
            script.append(f"(document.querySelector({json.dumps(sel)})||{{click(){{}}}}).click()")
        if hover:
            script.append(f"({HOVER_JS})({json.dumps(hover)})")
        inject = (f"<style>html,body{{width:{width}px;height:{height}px}}</style>"
                  f"<script>window.addEventListener('load',function(){{setTimeout(function(){{{';'.join(script)}}},{700 if demo else 0})}})</script></body>")
        html = root / page
        html.write_text(html.read_text(encoding="utf-8").replace("</body>", inject), encoding="utf-8")
        url = html.as_uri() + (f"?demo={demo}" if demo else "")
        out = out.resolve()
        subprocess.run(
            [str(edge), "--headless", "--disable-gpu", "--hide-scrollbars", f"--screenshot={out}",
             f"--window-size={width},{height}", "--virtual-time-budget=4000", url],
            capture_output=True, timeout=60,
        )
    if not out.exists():
        raise SystemExit("screenshot failed")
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out", type=Path)
    ap.add_argument("--state", type=Path, help="JSON payload as sent by the app")
    ap.add_argument("--demo", help="demo state instead of a state (picks, status, ...)")
    ap.add_argument("--page", default="index.html", help="page inside panel/: index.html or game.html")
    ap.add_argument("--explore", type=Path, help="JSON blind-pick report to draw in the explorer section")
    ap.add_argument("--click", action="append", help="CSS selector to click after rendering")
    ap.add_argument("--width", type=int, default=300)
    ap.add_argument("--height", type=int, default=900)
    ap.add_argument("--scale", type=float, default=1.0)
    ap.add_argument("--mode", default="")
    ap.add_argument("--hover", help="CSS selector to hover: shows its tooltip and a cursor")
    a = ap.parse_args()
    state = json.loads(a.state.read_text(encoding="utf-8")) if a.state else None
    explore = json.loads(a.explore.read_text(encoding="utf-8")) if a.explore else None
    print(render(a.out, state, a.demo if not state else None, a.click, a.width, a.height, a.scale, a.mode,
                 a.hover, a.page, explore))


if __name__ == "__main__":
    main()
