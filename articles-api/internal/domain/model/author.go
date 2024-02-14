package model

type Author struct {
	Id       int64
	Username string
	Articles []Article
}
