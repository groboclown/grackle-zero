
SUBDIRS := $(wildcard */.)

all: $(SUBDIRS) .FORCE

$(SUBDIRS): .FORCE
	$(MAKE) -C $(@D)

clean: .FORCE
	for dir in $(SUBDIRS); do \
		$(MAKE) -C $$dir clean; \
	done

format: .FORCE
	for dir in $(SUBDIRS); do \
		$(MAKE) -C $$dir format; \
	done

update: .FORCE
	for dir in $(SUBDIRS); do \
		test -f $$dir/Cargo.toml && cargo update; \
	done

.FORCE:
