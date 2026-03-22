// Expected: exit 1, check=placeholder-code
package main

func notDone() {
	panic("not implemented")
}

func alsoNotDone() {
	panic("TODO: implement this")
}

func main() {
	_ = notDone
	_ = alsoNotDone
}
