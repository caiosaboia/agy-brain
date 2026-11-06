"""
CLI entry point and command router for agy-brain.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from agy_brain import __version__
from agy_brain.core import (
    fetch_sessions,
    get_brain_root,
    get_db_path,
    get_default_app_dir,
    get_pinned_id_for_workspace,
    remove_pin,
    resolve_brain_target,
    set_pin,
)
from agy_brain.shells import get_shell_integration, install_shell_integration
from agy_brain.ui import (
    CYAN,
    GREEN,
    RED,
    RESET,
    WHITE,
    YELLOW,
    configure_windows_utf8,
    list_artifacts,
    open_in_editor,
    open_in_file_manager,
    render_preview,
    render_table,
)


def run_doctor(app_dir: Path | None = None) -> None:
    """Diagnoses environment health, database connection, and tool availability."""
    target_app = app_dir or get_default_app_dir()
    db_file = get_db_path(target_app)
    brain_dir = get_brain_root(target_app)

    print(f"\n{CYAN}+------------------------------------------------------------------------+{RESET}")
    print(f"{CYAN}| AGY-BRAIN DOCTOR - DIAGNOSTICO DO SISTEMA                             |{RESET}")
    print(f"{CYAN}+------------------------------------------------------------------------+{RESET}\n")

    # 1. App directory
    if target_app.is_dir():
        print(f"  {GREEN}[OK]{RESET} Diretorio Antigravity: {target_app}")
    else:
        print(f"  {RED}[X]{RESET}  Diretorio Antigravity nao encontrado: {target_app}")

    # 2. Database
    if db_file.is_file():
        print(f"  {GREEN}[OK]{RESET} Banco de dados: {db_file}")
        sessions = fetch_sessions(app_dir=target_app, all_workspaces=True)
        print(f"  {GREEN}[OK]{RESET} Total de sessoes registradas: {len(sessions)}")
    else:
        print(f"  {RED}[X]{RESET}  Banco SQLite nao encontrado em: {db_file}")

    # 3. Brain storage
    if brain_dir.is_dir():
        print(f"  {GREEN}[OK]{RESET} Pasta de cerebros: {brain_dir}")
    else:
        print(f"  {YELLOW}[!]{RESET} Pasta de cerebros ainda nao existe: {brain_dir}")

    # 4. Tools
    import shutil
    fzf_path = shutil.which("fzf")
    if fzf_path:
        print(f"  {GREEN}[OK]{RESET} fzf detectado: {fzf_path}")
    else:
        print(f"  {YELLOW}[!]{RESET} fzf nao encontrado no PATH (o modo TUI utilizara fallback tabular)")

    editor = os.environ.get("VISUAL") or os.environ.get("EDITOR")
    if not editor:
        for c in ["nvim", "code", "nano", "vim", "notepad"]:
            if shutil.which(c):
                editor = c
                break
    if editor:
        print(f"  {GREEN}[OK]{RESET} Editor de texto detectado: {editor}")
    else:
        print(f"  {YELLOW}[!]{RESET} Nenhum editor detectado no PATH")

    # 5. Current workspace pin
    cur_pin = get_pinned_id_for_workspace(os.getcwd(), target_app)
    if cur_pin:
        print(f"  {GREEN}[OK]{RESET} Workspace atual fixado no cerebro: {cur_pin}")
    else:
        print(f"  {WHITE}[i]{RESET} Workspace atual sem cerebro fixado (usando resolucao dinamica)")

    print(f"\n{CYAN}+------------------------------------------------------------------------+{RESET}\n")


def main() -> None:
    configure_windows_utf8()
    parser = argparse.ArgumentParser(
        prog="agy-brain",
        description="Universal, cross-platform CLI navigator and manager for Antigravity Brains.",
    )
    parser.add_argument("-v", "--version", action="version", version=f"%(prog)s {__version__}")
    parser.add_argument("--app-dir", help="Override Antigravity application directory", default=None)

    subparsers = parser.add_subparsers(dest="command")

    # go
    go_parser = subparsers.add_parser("go", help="Navigate to brain directory")
    go_parser.add_argument("target", nargs="?", default="default", help="Brain index or UUID")

    # list
    list_parser = subparsers.add_parser("list", aliases=["ls"], help="List brain sessions")
    list_parser.add_argument("-a", "--all", action="store_true", help="List all sessions globally")

    # info
    info_parser = subparsers.add_parser("info", aliases=["show"], help="Display session details")
    info_parser.add_argument("target", nargs="?", default="default")

    # artifacts
    art_parser = subparsers.add_parser("artifacts", aliases=["art"], help="List artifacts in brain")
    art_parser.add_argument("target", nargs="?", default="default")

    # open
    open_parser = subparsers.add_parser("open", help="Open brain folder in file manager")
    open_parser.add_argument("target", nargs="?", default="default")

    # edit
    edit_parser = subparsers.add_parser("edit", help="Open brain folder in code editor")
    edit_parser.add_argument("target", nargs="?", default="default")

    # pin
    pin_parser = subparsers.add_parser("pin", help="Pin brain session to current workspace")
    pin_parser.add_argument("target", nargs="?", default="default")

    # unpin
    subparsers.add_parser("unpin", help="Remove pin for current workspace")

    # resolve
    res_parser = subparsers.add_parser("resolve", help="Print absolute brain folder path")
    res_parser.add_argument("target", nargs="?", default="default")

    # fzf-lines
    fzf_parser = subparsers.add_parser("fzf-lines", help="Generate tab-separated lines for fzf")
    fzf_parser.add_argument("-a", "--all", action="store_true")

    # preview
    prev_parser = subparsers.add_parser("preview", help="Render preview card for fzf")
    prev_parser.add_argument("brain_id")

    # init
    init_parser = subparsers.add_parser("init", help="Generate shell integration code")
    init_parser.add_argument("shell", choices=["bash", "zsh", "fish", "powershell", "pwsh"])

    # install
    inst_parser = subparsers.add_parser("install", help="Install shell integration into RC/profile")
    inst_parser.add_argument("-s", "--shell", default=None, help="Target shell (bash, zsh, fish, pwsh)")

    # doctor
    subparsers.add_parser("doctor", help="Check system health and tools")

    # Parse arguments
    args = parser.parse_args()
    app_dir = Path(args.app_dir).resolve() if args.app_dir else None
    cur_ws = os.getcwd()

    if not args.command:
        # Default with no subcommands: if in a terminal, print list or launch
        sessions = fetch_sessions(cur_ws, all_workspaces=False, app_dir=app_dir)
        if not sessions:
            sessions = fetch_sessions(cur_ws, all_workspaces=True, app_dir=app_dir)
        render_table(sessions)
        return

    cmd = args.command

    if cmd == "resolve":
        bid = resolve_brain_target(args.target, cur_ws, app_dir)
        if bid:
            brain_path = get_brain_root(app_dir) / bid
            print(str(brain_path))
            sys.exit(0)
        sys.exit(1)

    elif cmd == "go":
        bid = resolve_brain_target(args.target, cur_ws, app_dir)
        if bid:
            brain_path = get_brain_root(app_dir) / bid
            print(str(brain_path))
            sys.exit(0)
        print(f"{RED}[!] Nenhum cerebro encontrado para resolver.{RESET}", file=sys.stderr)
        sys.exit(1)

    elif cmd in ["list", "ls"]:
        sessions = fetch_sessions(cur_ws, all_workspaces=args.all, app_dir=app_dir)
        render_table(sessions)

    elif cmd in ["info", "show"]:
        bid = resolve_brain_target(args.target, cur_ws, app_dir)
        if not bid:
            print(f"{RED}[!] Cerebro nao encontrado.{RESET}", file=sys.stderr)
            sys.exit(1)
        sessions = fetch_sessions(cur_ws, all_workspaces=True, app_dir=app_dir)
        match_sess = next((s for s in sessions if s["id"] == bid), None)
        render_preview(match_sess, bid, app_dir)

    elif cmd in ["artifacts", "art"]:
        bid = resolve_brain_target(args.target, cur_ws, app_dir)
        if not bid:
            print(f"{RED}[!] Cerebro nao encontrado.{RESET}", file=sys.stderr)
            sys.exit(1)
        list_artifacts(bid, app_dir)

    elif cmd == "open":
        bid = resolve_brain_target(args.target, cur_ws, app_dir)
        if not bid:
            print(f"{RED}[!] Cerebro nao encontrado.{RESET}", file=sys.stderr)
            sys.exit(1)
        brain_path = str(get_brain_root(app_dir) / bid)
        if open_in_file_manager(brain_path):
            print(f"{GREEN}[OK] Abrindo cerebro no gerenciador de arquivos: {bid}{RESET}")
        else:
            print(f"{RED}[!] Falha ao abrir pasta do cerebro: {brain_path}{RESET}", file=sys.stderr)

    elif cmd == "edit":
        bid = resolve_brain_target(args.target, cur_ws, app_dir)
        if not bid:
            print(f"{RED}[!] Cerebro nao encontrado.{RESET}", file=sys.stderr)
            sys.exit(1)
        brain_path = str(get_brain_root(app_dir) / bid)
        if not open_in_editor(brain_path):
            print(f"{RED}[!] Falha ao abrir pasta no editor: {brain_path}{RESET}", file=sys.stderr)

    elif cmd == "pin":
        bid = resolve_brain_target(args.target, cur_ws, app_dir)
        if not bid:
            print(f"{RED}[!] Informe um ID de sessao valido para fixar.{RESET}", file=sys.stderr)
            sys.exit(1)
        set_pin(bid, cur_ws, app_dir)
        print(f"{GREEN}[OK] Cerebro {bid} fixado com sucesso para este workspace!{RESET}")

    elif cmd == "unpin":
        if remove_pin(cur_ws, app_dir):
            print(f"{YELLOW}[OK] Fixacao removida do workspace atual.{RESET}")
        else:
            print(f"{WHITE}[i] Nenhuma fixacao encontrada para este workspace.{RESET}")

    elif cmd == "fzf-lines":
        sessions = fetch_sessions(cur_ws, all_workspaces=args.all, app_dir=app_dir)
        if not sessions and not args.all:
            sessions = fetch_sessions(cur_ws, all_workspaces=True, app_dir=app_dir)

        for s in sessions:
            tag = "[FIXADO]" if s.get("pinned") else "        "
            steps_str = f"{s.get('steps', 0):>5} passos"
            date_str = (s.get("mtime") or "")[:10]
            title = s.get("title") or "(sem titulo)"
            if len(title) > 42:
                title = title[:39] + "..."
            print(f"{s['id']}\t{tag}\t{title:<42}\t{steps_str}\t{date_str}")

    elif cmd == "preview":
        bid = args.brain_id
        sessions = fetch_sessions(cur_ws, all_workspaces=True, app_dir=app_dir)
        match_sess = next((s for s in sessions if s["id"] == bid), None)
        render_preview(match_sess, bid, app_dir)

    elif cmd == "init":
        code = get_shell_integration(args.shell)
        if code:
            print(code)
        else:
            print(f"Shell desconhecida: {args.shell}", file=sys.stderr)
            sys.exit(1)

    elif cmd == "install":
        success, msg = install_shell_integration(args.shell)
        if success:
            print(f"{GREEN}[OK] {msg}{RESET}")
        else:
            print(f"{RED}[X] {msg}{RESET}", file=sys.stderr)
            sys.exit(1)

    elif cmd == "doctor":
        run_doctor(app_dir)


if __name__ == "__main__":
    main()
