
SUBDIRS := $(wildcard */.)

all: $(SUBDIRS) .FORCE

$(SUBDIRS): .FORCE
	$(MAKE) -C $(@D)


.FORCE:
