package model

import "time"

type Article struct {
	// Should be non-zero if in database
	id              int64
	Title           string
	MarkdownContent string
	CoverImage      url
	created         time.Time
	Author          Author
}

type ArticleState interface {
	State() string
}
