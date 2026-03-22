// Expected: exit 0
// `if err == nil` guard pattern is not an empty error check.
package main

import "fmt"

func process(data *string) {
	if data == nil {
		return
	}
	fmt.Println(*data)
}

func main() {
	s := "hello"
	process(&s)
	process(nil)
}
