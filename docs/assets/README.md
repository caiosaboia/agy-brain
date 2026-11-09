# Assets de Demonstração e Mídia

Esta pasta foi configurada para armazenar vídeos, gravações e imagens de demonstração do projeto para exibição no `README.md`.

## Formatos Recomendados

- **Vídeo MP4 / WebM:** `demo.mp4` ou `demo.webm` (h.264 / AAC ou VP9 / Opus).
- **Animação GIF:** `demo.gif` (caso prefira reprodução automática em qualquer cliente markdown).

## Recomendações de Tamanho e Performance

- O GitHub impõe um limite máximo estrito de 100 MB por arquivo para pushes no Git tradicional.
- Para manter a clonagem rápida do repositório, recomenda-se que arquivos de vídeo fiquem abaixo de 20 MB a 30 MB.
- Alternativamente, você pode fazer upload do vídeo diretamente em uma Release do GitHub ou em um Issue/PR e usar a URL gerada (`https://github.com/user-attachments/assets/...`) dentro do `README.md`.

## Exemplos de Inclusão no README.md

### 1. Tag de Vídeo HTML (Renderiza player nativo no GitHub)
```html
<p align="center">
  <video src="docs/assets/demo.mp4" controls="controls" width="100%" style="max-width: 800px;">
    Seu navegador não suporta a tag de vídeo.
  </video>
</p>
```

### 2. Imagem GIF
```markdown
![Demonstração do Brain](docs/assets/demo.gif)
```
