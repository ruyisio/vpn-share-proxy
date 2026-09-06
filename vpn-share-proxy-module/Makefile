.PHONY: all build clean push

MODULE_NAME = vpn-share-proxy
ZIP_FILE = $(MODULE_NAME).zip

MODULE_FILES = \
	META-INF \
	module.prop \
	service.sh \
	customize.sh \
	bin

all: build

build: $(ZIP_FILE)

$(ZIP_FILE): $(shell find $(MODULE_FILES) -type f 2>/dev/null) module.prop service.sh customize.sh
	@echo "Packaging $(ZIP_FILE)..."
	@rm -f $(ZIP_FILE)
	@zip -r $(ZIP_FILE) $(MODULE_FILES) -x '*.git*' '*.DS_Store'
	@echo "Done! $(ZIP_FILE) is ready for flashing in KernelSU/Magisk."

clean:
	@rm -f $(ZIP_FILE)

push: build
	@adb push $(ZIP_FILE) /sdcard/
	@echo "Pushed $(ZIP_FILE) to /sdcard/ on device."
