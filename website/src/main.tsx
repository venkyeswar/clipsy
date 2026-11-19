import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@fontsource-variable/familjen-grotesk'
import '@fontsource-variable/onest'
import '@fontsource-variable/jetbrains-mono'
import './index.css'
import App from './App.tsx'
import { ThemeProvider } from './components/theme-provider.tsx'
import { PlatformProvider } from './lib/platform-context.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ThemeProvider>
      <PlatformProvider>
        <App />
      </PlatformProvider>
    </ThemeProvider>
  </StrictMode>,
)
