package main

import "github.com/calmbackup/cb-cli/cmd"

var version = "dev"

func main() {
	cmd.Execute(version)
}
