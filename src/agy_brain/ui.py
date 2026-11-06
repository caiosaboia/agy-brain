"""
Terminal UI, ANSI colors, tables, preview cards and cross-platform launchers.
"""

from __future__ import annotations

import io
import os
import platform
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

from agy_brain.core import get_brain_stats


def configure_windows_utf8() -> None:
    """Configures UTF-8 encoding on Windows console when running directly in terminal."""
    if "pytest" in sys.modules:
        return
    if sys.platform == "win32":
        try:
            if hasattr(sys.stdout, "buffer"):
                sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
            if hasattr(sys.stderr, "buffer"):
                sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")
        except Exception:
            pass


def supports_color() -> bool:
    """Checks whether the current terminal supports ANSI colors."""
    if os.environ.get("NO_COLOR"):
        return False
    if not hasattr(sys.stdout, "isatty") or not sys.stdout.isatty():
        return False
    return True


USE_COLOR = supports_color()

# ANSI Color Codes
RESET = "\033[0m" if USE_COLOR else ""
BOLD = "\033[1m" if USE_COLOR else ""
DIM = "\033[2m" if USE_COLOR else ""
CYAN = "\033[96m" if USE_COLOR else ""
GREEN = "\033[92m" if USE_COLOR else ""
YELLOW = "\033[93m" if USE_COLOR else ""
BLUE = "\033[94m" if USE_COLOR else ""
MAGENTA = "\033[95m" if USE_COLOR else ""
WHITE = "\033[97m" if USE_COLOR else ""
RED = "\033[91m" if USE_COLOR else ""


def render_preview(
    session: dict[str, Any] | None,
    brain_id: str,
    app_dir: Path | None = None,
) -> None:
    """Prints the rich preview card used by fzf or 'agy-brain info'."""
    stats = get_brain_stats(brain_id, app_dir)

    print(f"{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}")
    print(f"{CYAN}{BOLD}| ANTIGRAVITY BRAIN INSPECTOR                                            |{RESET}")
    print(f"{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}\n")

    if session:
        tag = f"{GREEN}{BOLD}[FIXADO]{RESET} " if session.get("pinned") else ""
        print(f"{WHITE}{BOLD}SESSAO{RESET}")
        print(f"  {DIM}Titulo:{RESET}     {BOLD}{session.get('title')}{RESET} {tag}")
        print(f"  {DIM}ID:{RESET}         {CYAN}{session.get('id')}{RESET}")
        print(f"  {DIM}Passos:{RESET}     {YELLOW}{session.get('steps')}{RESET} interacoes")
        print(f"  {DIM}Status:{RESET}     {session.get('status') or 'N/A'}")
        print(f"  {DIM}Modificado:{RESET} {session.get('mtime') or 'N/A'}")
        if session.get("primary_workspace"):
            clean_ws = session["primary_workspace"].replace("file:///", "")
            print(f"  {DIM}Workspace:{RESET}  {clean_ws}")
    else:
        print(f"{WHITE}{BOLD}SESSAO{RESET}")
        print(f"  {DIM}ID:{RESET}         {CYAN}{brain_id}{RESET}")

    print()
    if stats and stats["exists"]:
        print(f"{WHITE}{BOLD}ESTATISTICAS DO CEREBRO{RESET}")
        print(f"  {DIM}Artefatos MD:{RESET}   {GREEN}{stats['md_count']}{RESET} planos/relatorios")
        print(f"  {DIM}Imagens PNG:{RESET}    {YELLOW}{stats['png_count']}{RESET} capturas")
        print(f"  {DIM}Scripts Scratch:{RESET}{stats['scratch_count']} arquivos")
        print(f"  {DIM}Caminho Local:{RESET}  {stats['path']}")
        print()

        if stats["recent_artifacts"]:
            print(f"{WHITE}{BOLD}ULTIMOS ARTEFATOS GERADOS{RESET}")
            for name, size, mtime in stats["recent_artifacts"]:
                kb = f"{size / 1024:.1f} KB"
                dt = datetime.fromtimestamp(mtime).strftime("%d/%m %H:%M")
                print(f"  * {BOLD}{name:<40}{RESET} {DIM}({kb}, {dt}){RESET}")
        else:
            print(f"{DIM}  Nenhum artefato markdown gerado nesta sessao.{RESET}")
    else:
        print(f"{RED}[!] Pasta do cerebro ainda nao foi criada ou nao contem arquivos no disco.{RESET}")

    print(f"\n{CYAN}--------------------------------------------------------------------------{RESET}")
    print(f"{DIM}Atalhos: {WHITE}[Enter]{DIM} Navegar | {WHITE}[Ctrl+O]{DIM} Explorer | {WHITE}[Ctrl+P]{DIM} Fixar | {WHITE}[Ctrl+A]{DIM} Todos | {WHITE}[Ctrl+E]{DIM} Editar{RESET}")


def render_table(sessions: list[dict[str, Any]]) -> None:
    """Prints sessions in a clean Unicode/ASCII table."""
    if not sessions:
        print(f"{YELLOW}[*] Nenhuma sessao de cerebro encontrada.{RESET}")
        return

    print(f"\n{CYAN}{BOLD}+----------------------------------------------------------------------------------------------------+{RESET}")
    print(f"{CYAN}{BOLD}| SESSOES DO ANTIGRAVITY BRAIN                                                                       |{RESET}")
    print(f"{CYAN}{BOLD}+-----+----------+------------------------------------------+--------------+------------+------------+{RESET}")
    print(f"{CYAN}{BOLD}| IDX | STATUS   | TITULO DA SESSAO                         | PASSOS       | DATA       | ID         |{RESET}")
    print(f"{CYAN}{BOLD}+-----+----------+------------------------------------------+--------------+------------+------------+{RESET}")

    for i, s in enumerate(sessions):
        tag = f"{GREEN}[FIXADO]{RESET}" if s.get("pinned") else "        "
        title = s.get("title") or "(sem titulo)"
        if len(title) > 40:
            title = title[:37] + "..."
        short_id = s.get("id", "")[:8] + "..."
        steps_str = f"{s.get('steps', 0)} p."
        mtime = (s.get("mtime") or "")[:10]
        print(f"| {i:<3} | {tag} | {title:<40} | {steps_str:<12} | {mtime:<10} | {short_id:<10} |")

    print(f"{CYAN}{BOLD}+-----+----------+------------------------------------------+--------------+------------+------------+{RESET}\n")


def list_artifacts(brain_id: str, app_dir: Path | None = None) -> None:
    """Displays user-facing artifacts inside the brain folder."""
    stats = get_brain_stats(brain_id, app_dir)
    if not stats or not stats["exists"]:
        print(f"{RED}[!] Cerebro {brain_id} nao encontrado no disco.{RESET}")
        return

    brain_path = Path(stats["path"])
    print(f"\n{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}")
    print(f"{CYAN}{BOLD}| ARTEFATOS DO CEREBRO: {brain_id[:16]}...                             |{RESET}")
    print(f"{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}")
    print(f"{DIM}Caminho: {brain_path}{RESET}\n")

    files: list[tuple[str, int, float, str]] = []

    # 1. Root artifacts
    try:
        for entry in brain_path.iterdir():
            if entry.is_file():
                name = entry.name
                if name.endswith(".metadata.json"):
                    continue
                try:
                    files.append((name, entry.stat().st_size, entry.stat().st_mtime, "root"))
                except Exception:
                    pass
    except Exception:
        pass

    # 2. Scratch artifacts
    scratch_dir = brain_path / "scratch"
    if scratch_dir.is_dir():
        try:
            for entry in scratch_dir.iterdir():
                if entry.is_file():
                    try:
                        rel_path = os.path.join("scratch", entry.name)
                        files.append((rel_path, entry.stat().st_size, entry.stat().st_mtime, "scratch"))
                    except Exception:
                        pass
        except Exception:
            pass

    files.sort(key=lambda x: -x[2])

    if not files:
        print(f"{YELLOW}[*] Nenhum artefato de usuario (.md, .png, scratch) gerado nesta sessao.{RESET}\n")
        return

    print(f"{WHITE}{BOLD}{'ARQUIVO':<48} {'TAMANHO':>10}  {'MODIFICADO'}{RESET}")
    print(f"{DIM}{'-'*48} {'-'*10}  {'-'*16}{RESET}")
    for rel, sz, mt, cat in files:
        dt = datetime.fromtimestamp(mt).strftime("%Y-%m-%d %H:%M")
        kb = f"{sz / 1024:.1f} KB"
        ext_color = WHITE
        if rel.endswith(".md"):
            ext_color = GREEN
        elif rel.endswith(".png"):
            ext_color = YELLOW
        elif cat == "scratch":
            ext_color = MAGENTA
        print(f"{ext_color}{rel:<48}{RESET} {DIM}{kb:>10}  {dt}{RESET}")

    print(f"\n{DIM}Total de artefatos visiveis: {len(files)}{RESET}\n")


def open_in_file_manager(target_path: str) -> bool:
    """Cross-platform opening in OS file manager (Explorer, Finder, xdg-open)."""
    p = Path(target_path).resolve()
    if not p.exists():
        return False

    system = platform.system()
    try:
        if system == "Windows":
            os.startfile(str(p))
            return True
        elif system == "Darwin":
            subprocess.run(["open", str(p)], check=False)
            return True
        else:
            subprocess.run(["xdg-open", str(p)], check=False)
            return True
    except Exception:
        return False


def open_in_editor(target_path: str) -> bool:
    """Cross-platform opening in user's configured code editor."""
    p = Path(target_path).resolve()
    if not p.exists():
        return False

    editor = os.environ.get("VISUAL") or os.environ.get("EDITOR")
    if not editor:
        for candidate in ["nvim", "code", "nano", "vim", "notepad"]:
            if shutil.which(candidate):
                editor = candidate
                break

    if not editor:
        return open_in_file_manager(str(p))

    try:
        subprocess.run([editor, str(p)], check=False)
        return True
    except Exception:
        return False
