// Expected: exit 1, check=empty-error-check
package main

import "os"

func main() {
	_, err := os.Open("file.txt")
	if err != nil {
	}
}
