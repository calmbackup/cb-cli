BINARY_NAME=calmbackup
VERSION?=dev
LDFLAGS=-ldflags "-X main.version=$(VERSION)"
GOBIN?=$(shell go env GOPATH)/bin

.PHONY: build test test-int lint ci clean

build:
	go build $(LDFLAGS) -o bin/$(BINARY_NAME) .

test:
	go test ./... -v

test-int:
	go test -tags=integration ./... -v

lint:
	golangci-lint run ./...

ci: lint test build

clean:
	rm -rf bin/
