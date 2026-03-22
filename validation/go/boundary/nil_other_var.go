// Expected: exit 1, check=empty-error-check
// Any nil-check with empty handling body should be caught, not just `err`.
package main

func process(result *int) {
	if result != nil {
	}
}

func main() {
	process(nil)
}
