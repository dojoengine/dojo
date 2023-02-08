# Won't write the called command in the console
.SILENT:
# Because we have a folder called test we need PHONY to avoid collision
.PHONY: test 

INSTALLATION_FOLDER=./cairo

install: 
	if [ -d $(INSTALLATION_FOLDER) ]; then \
		$(MAKE) update-cairo; \
	else \
		$(MAKE) clone-cairo; \
	fi
	$(MAKE) build

clone-cairo:
	mkdir -p $(INSTALLATION_FOLDER)
	git clone --depth 1 https://github.com/starkware-libs/cairo.git $(INSTALLATION_FOLDER)


update-cairo:
	git -C $(INSTALLATION_FOLDER) pull

build:
	cargo run --bin dojo-cli -- build ${dir}

language-server:
	cargo build --bin dojo-language-server --release