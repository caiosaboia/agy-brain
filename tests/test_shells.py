"""
Unit tests for agy_brain.shells.
"""

from pathlib import Path

from agy_brain.shells import (
    get_shell_integration,
    install_shell_integration,
)


def test_get_shell_integration():
    bash_code = get_shell_integration("bash")
    assert bash_code is not None
    assert "agy-brain() {" in bash_code
    assert "cd-brain" in bash_code

    zsh_code = get_shell_integration("zsh")
    assert zsh_code is not None
    assert "agy-brain() {" in zsh_code

    fish_code = get_shell_integration("fish")
    assert fish_code is not None
    assert "function agy-brain" in fish_code

    pwsh_code = get_shell_integration("powershell")
    assert pwsh_code is not None
    assert "function agy-brain {" in pwsh_code

    assert get_shell_integration("unknown_shell") is None


def test_install_shell_integration_idempotence(tmp_path, monkeypatch):
    mock_home = tmp_path / "home"
    mock_home.mkdir()
    monkeypatch.setattr(Path, "home", lambda: mock_home)

    # Test bash install
    success, msg = install_shell_integration("bash")
    assert success is True
    bashrc = mock_home / ".bashrc"
    assert bashrc.is_file()
    content = bashrc.read_text(encoding="utf-8")
    assert "agy-brain init bash" in content

    # Run install again (should be idempotent)
    success2, msg2 = install_shell_integration("bash")
    assert success2 is True
    assert "ja esta instalada" in msg2.lower()
    content2 = bashrc.read_text(encoding="utf-8")
    assert content2 == content
