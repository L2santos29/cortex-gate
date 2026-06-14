# ==========================================================================
# Cortex Gate — Makefile
#
# Targets for building, launching, and managing the Cortex Gate ecosystem.
# ==========================================================================

.PHONY: all ui build-ui launch-ui install-desktop help

all: help

# ── UI (Desktop Application) ────────────────────────────────────────────────

ui: launch-ui       ## Build and launch the Tauri desktop UI

build-ui:           ## Build the Tauri desktop binary (release)
	cd frontend && npm run build
	cd frontend/src-tauri && cargo build --release

launch-ui:          ## Launch the UI (builds first if needed)
	./launch-ui.sh

install-desktop:    ## Install the .desktop file for app-menu access
	@mkdir -p $(HOME)/.local/share/applications
	@sed 's|/home/l2s/Documents/L&S Agent/agency projects/cortex-gate|$(CURDIR)|g' \
		cortex-gate.desktop > $(HOME)/.local/share/applications/cortex-gate.desktop
	@chmod +x $(HOME)/.local/share/applications/cortex-gate.desktop
	@echo "✅ Desktop entry installed. Find 'Cortex Gate' in your app menu."

copy-to-desktop:    ## Copy .desktop file to ~/Desktop for double-click
	@mkdir -p $(HOME)/Desktop
	@cp cortex-gate.desktop $(HOME)/Desktop/cortex-gate.desktop
	@chmod +x $(HOME)/Desktop/cortex-gate.desktop
	@echo "✅ Copied to ~/Desktop/cortex-gate.desktop"
	@echo "   Double-click it to launch Cortex Gate."
	@echo "   (You may need to right-click → 'Allow Launching' on first use)"

# ── Gateway Server ──────────────────────────────────────────────────────────

serve:              ## Run the gateway server (backend)
	cargo run --release

# ── Development & Maintenance ───────────────────────────────────────────────

check:              ## Check all Rust code compiles
	cargo check
	cargo check --release

test:               ## Run all tests
	cargo test

clean:              ## Clean build artifacts
	cargo clean
	cd frontend && rm -rf dist node_modules/.vite
	cd frontend/src-tauri && cargo clean

# ── Help ────────────────────────────────────────────────────────────────────

help:               ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Quick start:"
	@echo "  make ui       — Launch the desktop UI"
	@echo "  make serve    — Run the gateway server"
	@echo "  make copy-to-desktop — Put launcher on your desktop"
