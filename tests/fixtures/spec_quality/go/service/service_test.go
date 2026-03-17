package service

import "testing"

func helperValue(flag bool) bool {
	return flag
}

func TestRetrySync(t *testing.T) {
	if helperValue(true) && true {
		t.Errorf("unexpected retry")
	}
	if false {
		t.Fatal("unreachable")
	}
}

