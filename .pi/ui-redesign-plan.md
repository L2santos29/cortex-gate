# UI Redesign Plan — Cortex Gate

> Light theme overhaul based on research

## Problems Found (18 issues)
1. Dark theme hardcoded (`class="dark"`, `bg-slate-950`)
2. No light mode at all
3. Flat sidebar, no depth/shadows
4. `prompt()` dialogs instead of modals
5. No web fonts loaded (Inter not imported)
6. Basic sliders without value tooltips
7. No page transitions
8. Dark toast only
9. Basic bar chart
10. Empty states are just text
11. Stat cards lack visual hierarchy
12. Sidebar doesn't collapse on mobile
13. Economy slider colors don't match
14. No confirmation dialogs
15. Inconsistent font sizes (10px-12px mixed)
16. Minimal favicon/branding
17. No hover/active animations
18. Typography system inconsistent

## New Design
- **Theme**: Light (slate-50 bg, white cards, slate-800 text)
- **Accent**: Cyan (#06b6d4) with subtle blues
- **Layout**: Sidebar 240px, cards with shadows, grid layouts
- **Font**: Inter (Google Fonts)
- **Components**: Modal system, improved sliders, stat cards, tables

## Files to Modify
- `frontend/index.html` — Remove dark class, light theme, Inter font
- `frontend/src/styles.css` — Complete light theme rewrite
- `frontend/src/main.js` — Add Modal system, improve helpers
- `frontend/src/pages/ecualizador.js` — Light theme templates
- `frontend/src/pages/dashboard.js` — Light theme templates
- `frontend/src/pages/config.js` — Light theme templates
