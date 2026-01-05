# Seed Engine WASM Demo

An interactive demo of the Seed rendering engine running in the browser via WebAssembly.

## Running the Demo

### Option 1: Python Server (Recommended)

```bash
cd seed-wasm
python demo/serve.py
```

This will start a local server and open the demo in your browser.

### Option 2: Any HTTP Server

The demo needs to be served over HTTP (not `file://`) due to WASM security requirements.

```bash
cd seed-wasm
npx serve .
# Then open http://localhost:3000/demo/
```

Or with Python:

```bash
cd seed-wasm
python -m http.server 8080
# Then open http://localhost:8080/demo/
```

## Features

- **Live Editor**: Write Seed markup and see results instantly
- **Render Preview**: Real-time PNG rendering via software rasterizer
- **Export Options**:
  - SVG export for vector graphics
  - PDF export for print
  - JSON export for AST inspection
- **Example Templates**: Load pre-built examples to explore the syntax

## Keyboard Shortcuts

- `Ctrl+Enter`: Render the current document

## Seed Syntax Examples

### Simple Frame

```
Frame
  width: 200px
  height: 100px
  fill: #4a90d9
  corner-radius: 8px
```

### With Design Tokens

```
tokens
  color.primary: #667eea
  spacing.md: 16px

Frame
  width: 200px
  height: 100px
  fill: $color.primary
```

### Gradients

```
Frame
  width: 200px
  height: 100px
  fill: linear-gradient(135deg, #667eea 0%, #764ba2 100%)
```

### Nested Frames

```
Frame
  width: 300px
  height: 200px
  fill: #2d2d44

  Frame
    width: 100px
    height: 80px
    x: 20px
    y: 20px
    fill: #667eea
```

## Browser Requirements

- Modern browser with WebAssembly support (Chrome, Firefox, Safari, Edge)
- JavaScript modules support
