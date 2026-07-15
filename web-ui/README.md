# SWE-CLI Web UI

Modern web interface for SWE-CLI built with React, TypeScript, and Tailwind CSS.

## Features

- 💬 Real-time chat with AI agent via WebSocket
- 📝 Markdown rendering for code blocks and formatted text
- 🎨 Dark theme optimized for coding
- ⚡ Fast and responsive interface
- 🔄 Automatic reconnection on disconnect
- 📊 Session management
- ⚙️ Configuration management

## Development

### Prerequisites

- Node.js 18+ or Bun
- SWE-CLI backend running

### Installation

```bash
cd web-ui
npm install
# or
bun install
```

### Running Dev Server

```bash
npm run dev
# or
bun run dev
```

The dev server will start on `http://localhost:5173` and proxy API requests to `http://localhost:8080`.

### Building for Production

```bash
npm run build
# or
bun run build
```

This will build the frontend and output to `../swecli/web/static/`, which will be served by the FastAPI backend.

## Architecture

### Tech Stack

- **React 18** - UI framework
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **Tailwind CSS** - Styling
- **Zustand** - State management
- **react-markdown** - Markdown rendering

### Structure

```
web-ui/
├── src/
│   ├── components/     # React components
│   │   ├── Chat/       # Chat interface
│   │   └── Layout/     # Layout components
│   ├── api/            # API client and WebSocket
│   ├── stores/         # Zustand state stores
│   ├── types/          # TypeScript types
│   ├── App.tsx         # Main app component
│   └── main.tsx        # Entry point
├── public/             # Static assets
└── index.html          # HTML template
```

### API Integration

The frontend communicates with the backend through:

1. **REST API** (`/api/*`) - For CRUD operations
   - Chat: `/api/chat/*`
   - Sessions: `/api/sessions/*`
   - Config: `/api/config/*`

2. **WebSocket** (`/ws`) - For real-time updates
   - Message streaming
   - Tool call notifications
   - Approval requests

### State Management

Uses Zustand for lightweight state management:

- `useChatStore` - Chat messages and WebSocket state

## Running with SWE-CLI

### Development Mode

1. Start SWE-CLI with UI enabled:
   ```bash
   swecli --enable-ui
   ```

2. In another terminal, start the Vite dev server:
   ```bash
   cd web-ui
   npm run dev
   ```

3. Open `http://localhost:5173` in your browser

### Production Mode

1. Build the frontend:
   ```bash
   cd web-ui
   npm run build
   ```

2. Start SWE-CLI with UI:
   ```bash
   swecli --enable-ui
   ```

3. Open `http://localhost:8080` in your browser

## Configuration

### Vite Configuration

The `vite.config.ts` includes:

- Proxy to backend API (`/api` → `http://localhost:8080`)
- Proxy to WebSocket (`/ws` → `ws://localhost:8080`)
- Build output to `../swecli/web/static/`

### Tailwind Configuration

Custom dark theme with:

- Custom scrollbar styling
- Code syntax highlighting
- Responsive design

## Troubleshooting

### WebSocket Connection Issues

If WebSocket fails to connect:

1. Ensure backend is running: `swecli --enable-ui`
2. Check browser console for connection errors
3. Verify port 8080 is not blocked by firewall

### Build Issues

If build fails:

1. Clear node_modules: `rm -rf node_modules && npm install`
2. Clear build cache: `rm -rf ../swecli/web/static`
3. Rebuild: `npm run build`

### Dev Server Not Proxying

If API requests fail in development:

1. Verify backend is running on port 8080
2. Check `vite.config.ts` proxy settings
3. Restart Vite dev server

## Future Features

- [ ] Session timeline viewer
- [ ] File tree browser
- [ ] Diff viewer
- [ ] Token usage analytics
- [ ] Multi-tab support
- [ ] Custom agent creation UI
- [ ] Approval rules editor

