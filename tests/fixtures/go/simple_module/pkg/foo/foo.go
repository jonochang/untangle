package foo

import "github.com/user/project/pkg/bar"

func Hello() string {
	return "Hello " + bar.World()
}
