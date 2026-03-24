package cmd

import (
	"fmt"

	"github.com/calmbackup/cb-cli/internal/backup"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

var selectedStyle = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("6"))

type pickerModel struct {
	backups  []backup.BackupEntry
	cursor   int
	selected *backup.BackupEntry
	quitting bool
}

func (m pickerModel) Init() tea.Cmd {
	return nil
}

func (m pickerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "up", "k":
			if m.cursor > 0 {
				m.cursor--
			} else {
				m.cursor = len(m.backups) - 1
			}
		case "down", "j":
			if m.cursor < len(m.backups)-1 {
				m.cursor++
			} else {
				m.cursor = 0
			}
		case "enter":
			m.selected = &m.backups[m.cursor]
			m.quitting = true
			return m, tea.Quit
		case "esc":
			m.quitting = true
			return m, tea.Quit
		}
	}
	return m, nil
}

func (m pickerModel) View() string {
	if m.quitting {
		return ""
	}

	s := "Select a backup to restore:\n\n"

	for i, b := range m.backups {
		line := fmt.Sprintf("  ● %s   %s   %s", b.Filename, formatSize(b.Size), formatTime(b.CreatedAt))
		if i == m.cursor {
			line = selectedStyle.Render(line)
		}
		s += line + "\n"
	}

	s += fmt.Sprintf("\n  ↑/↓ navigate • enter select • esc quit\n  Showing %d most recent backups\n", len(m.backups))

	return s
}

func runPicker(backups []backup.BackupEntry) (*backup.BackupEntry, error) {
	if len(backups) == 0 {
		return nil, fmt.Errorf("no backups available")
	}

	model := pickerModel{backups: backups}
	p := tea.NewProgram(model)
	result, err := p.Run()
	if err != nil {
		return nil, err
	}

	final := result.(pickerModel)
	return final.selected, nil
}
