# Tauri and React for the app

CopilotBar is a Tauri app, with a Rust core and a React UI, rather than a native macOS app. We chose it for two reasons. Web tooling makes UI iteration much faster, including previewing every view in a browser with demo data. Charting is also easier in the React ecosystem. Tauri also keeps Windows and Linux support possible later, but we are not designing for it now.
