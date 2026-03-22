// Expected: exit 1, check=unreachable-code
package main

import "fmt"

func process() int {
	return 42
	fmt.Println("never reached")
	return 0
}

func main() {
	_ = process()
}
