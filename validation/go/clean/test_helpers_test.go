// Expected: exit 0
package main

import "testing"

func TestAdd(t *testing.T) {
	result := 1 + 2
	if result != 3 {
		t.Errorf("expected 3, got %d", result)
	}
}
