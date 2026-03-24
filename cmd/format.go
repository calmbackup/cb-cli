package cmd

import (
	"fmt"
	"time"
)

func formatTime(isoTime string) string {
	t, err := time.Parse(time.RFC3339, isoTime)
	if err != nil {
		return isoTime
	}

	readable := t.Local().Format("Jan 2, 2006 15:04")

	d := time.Since(t)
	var ago string
	switch {
	case d < time.Minute:
		ago = "just now"
	case d < time.Hour:
		ago = fmt.Sprintf("%dm ago", int(d.Minutes()))
	case d < 24*time.Hour:
		ago = fmt.Sprintf("%dh ago", int(d.Hours()))
	case d < 30*24*time.Hour:
		ago = fmt.Sprintf("%dd ago", int(d.Hours()/24))
	default:
		months := int(d.Hours() / 24 / 30)
		if months == 1 {
			ago = "1 month ago"
		} else {
			ago = fmt.Sprintf("%d months ago", months)
		}
	}

	return fmt.Sprintf("%s (%s)", readable, ago)
}
