
SUBDIRS := $(wildcard */.)

all: $(SUBDIRS) .FORCE

$(SUBDIRS): .FORCE
	$(MAKE) -C $(@D)

clean: .FORCE
	for dir in $(SUBDIRS); do \
		$(MAKE) -C $$dir clean; \
	done

.FORCE:
