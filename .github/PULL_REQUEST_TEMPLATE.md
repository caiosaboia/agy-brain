## Resumo das Alteracoes

Descreva de forma objetiva o que este Pull Request implementa, corrige ou refatora, e qual a motivacao da mudanca.

## Tipo de Mudanca

- [ ] `feat`: Nova funcionalidade (CLI / TUI / Core / Shell Hook)
- [ ] `fix`: Correcao de bug
- [ ] `refactor`: Refatoracao interna sem alteracao de contrato externo
- [ ] `test`: Adicao ou melhoria de testes automatizados
- [ ] `ci`: Alteracao em workflows do GitHub Actions ou scripts de build
- [ ] `docs`: Atualizacao de documentacao

## Checklist de Qualidade (Quality Gate)

- [ ] Formatacao verificada com `cargo fmt --all -- --check`
- [ ] Linter aprovado sem avisos com `cargo clippy --all-targets -- -D warnings`
- [ ] Todos os testes (Core, Shells, UI, Headless TUI e CLI E2E) passando com `cargo test --all-targets`
- [ ] Compatibilidade multiplataforma preservada (Windows, Linux e macOS)
- [ ] Convencoes do projeto respeitadas (codigo e commits em ingles, zero emojis)
