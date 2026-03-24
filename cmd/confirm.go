package cmd

import (
	"fmt"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

var (
	activeButtonStyle   = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("6"))
	inactiveButtonStyle = lipgloss.NewStyle().Faint(true)
)

type confirmModel struct {
	message   string
	cursor    int // 0 = No, 1 = Yes
	confirmed bool
	done      bool
}

func (m confirmModel) Init() tea.Cmd {
	return nil
}

func (m confirmModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "left", "h", "tab", "right", "l":
			if m.cursor == 0 {
				m.cursor = 1
			} else {
				m.cursor = 0
			}
		case "enter":
			m.confirmed = m.cursor == 1
			m.done = true
			return m, tea.Quit
		case "esc":
			m.confirmed = false
			m.done = true
			return m, tea.Quit
		}
	}
	return m, nil
}

func (m confirmModel) View() string {
	if m.done {
		return ""
	}

	s := fmt.Sprintf("%s%s\n", pad, m.message)
	s += pad + "This will overwrite your current database.\n\n"

	noLabel := "[ No ]"
	yesLabel := "[ Yes ]"

	if m.cursor == 0 {
		noLabel = activeButtonStyle.Render(noLabel)
		yesLabel = inactiveButtonStyle.Render(yesLabel)
	} else {
		noLabel = inactiveButtonStyle.Render(noLabel)
		yesLabel = activeButtonStyle.Render(yesLabel)
	}

	s += fmt.Sprintf("%s%s  %s\n", pad, noLabel, yesLabel)

	return s
}

func runConfirm(filename string) (bool, error) {
	model := confirmModel{
		message: fmt.Sprintf("Restore %s?", filename),
		cursor:  0,
	}
	p := tea.NewProgram(model)
	result, err := p.Run()
	if err != nil {
		return false, err
	}

	final := result.(confirmModel)
	return final.confirmed, nil
}
