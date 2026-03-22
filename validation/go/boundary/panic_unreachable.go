// Expected: exit 0
// Known limitation: panic() is not detected as terminal (only return is).
package main

import "fmt"

func fatal() {
	panic("fatal error")
	fmt.Println("after panic")
}

func main() {
	fatal()
}
