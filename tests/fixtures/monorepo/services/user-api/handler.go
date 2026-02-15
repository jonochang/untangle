package handler

import "fmt"

func Start() {
	fmt.Println("User API started")
}

func GetUser(id string) string {
	return id
}
