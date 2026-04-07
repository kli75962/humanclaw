import { useState, useMemo } from 'react';
import { ChevronLeft } from 'lucide-react';
import '../../style/BirthdayCalendar.css';

interface BirthdayCalendarProps {
  value: string | null;
  onChange: (date: string) => void;
}

type ViewMode = 'year' | 'month' | 'day';

export function BirthdayCalendar({ value, onChange }: BirthdayCalendarProps) {
  const today = useMemo(() => new Date(), []);
  const [viewMode, setViewMode] = useState<ViewMode>(() => {
    // If there's a date already selected, start in day view; otherwise start in year view
    return value ? 'day' : 'year';
  });
  const [yearRangeOffset, setYearRangeOffset] = useState(0); // 0 = current decade area, negative = earlier decades
  const [selectedYear, setSelectedYear] = useState(() => {
    if (value) {
      const [year] = value.split('-');
      return parseInt(year);
    }
    return today.getFullYear();
  });
  const [selectedMonth, setSelectedMonth] = useState(() => {
    if (value) {
      const [, month] = value.split('-');
      return parseInt(month) - 1;
    }
    return today.getMonth();
  });

  const currentYear = today.getFullYear();
  const baseYearRangeStart = Math.floor(currentYear / 10) * 10 - 10;

  // Calculate offset to focus on selected year if a date is set
  const focusedYearRangeOffset = useMemo(() => {
    if (!value) return 0;
    const [year] = value.split('-');
    const yearNum = parseInt(year);
    const baseRange = Math.floor(yearNum / 20) * 20;
    return (baseRange - baseYearRangeStart) / 20;
  }, [value, baseYearRangeStart]);

  const effectiveYearRangeOffset = value && viewMode !== 'year' ? focusedYearRangeOffset : yearRangeOffset;
  const yearRangeStart = baseYearRangeStart + effectiveYearRangeOffset * 20;
  const yearRangeEnd = yearRangeStart + 20;

  const daysInMonth = (month: number, year: number) => new Date(year, month + 1, 0).getDate();
  const firstDayOfMonth = (month: number, year: number) => new Date(year, month, 1).getDay();

  const monthDays = useMemo(() => {
    const days = [];
    const startDay = firstDayOfMonth(selectedMonth, selectedYear);
    const totalDays = daysInMonth(selectedMonth, selectedYear);

    for (let i = 0; i < startDay; i++) {
      days.push(null);
    }
    for (let i = 1; i <= totalDays; i++) {
      days.push(i);
    }
    return days;
  }, [selectedYear, selectedMonth]);

  const isDateDisabled = (day: number) => {
    const date = new Date(selectedYear, selectedMonth, day);
    return date > today;
  };

  const isDateSelected = (day: number) => {
    if (!value) return false;
    const [year, month, dayStr] = value.split('-');
    return (
      selectedYear === parseInt(year) &&
      selectedMonth === parseInt(month) - 1 &&
      day === parseInt(dayStr)
    );
  };

  const handleYearSelect = (year: number) => {
    if (year > currentYear) return; // Don't allow future years
    setSelectedYear(year);
    setViewMode('month');
  };

  const handleMonthSelect = (month: number) => {
    // Check if this month is in the future
    if (selectedYear === currentYear && month > today.getMonth()) return;
    setSelectedMonth(month);
    setViewMode('day');
  };

  const handleDaySelect = (day: number) => {
    if (isDateDisabled(day)) return;
    const month = String(selectedMonth + 1).padStart(2, '0');
    const dayStr = String(day).padStart(2, '0');
    onChange(`${selectedYear}-${month}-${dayStr}`);
  };

  const handleBackToYear = () => {
    setViewMode('year');
  };

  const handleBackToMonth = () => {
    setViewMode('month');
  };

  const handlePrevYearRange = () => {
    // Don't go before 1900
    if (yearRangeStart - 20 >= 1900) {
      setYearRangeOffset((prev) => prev - 1);
    }
  };

  const handleNextYearRange = () => {
    // Don't go beyond the current year
    if (yearRangeStart + 20 <= currentYear) {
      setYearRangeOffset((prev) => prev + 1);
    }
  };

  const monthName = new Date(selectedYear, selectedMonth).toLocaleDateString('en-US', { month: 'long' });

  return (
    <div className="birthday-calendar">
      {viewMode === 'year' && (
        <>
          <div className="birthday-calendar-header">
            <button
              className="birthday-calendar-back-btn"
              onClick={handlePrevYearRange}
              disabled={yearRangeStart - 20 < 1900}
              aria-label="Earlier years"
            >
              <ChevronLeft size={16} />
            </button>
            <div className="birthday-calendar-title">
              {yearRangeStart} – {yearRangeEnd - 1}
            </div>
            <button
              className="birthday-calendar-back-btn"
              onClick={handleNextYearRange}
              disabled={yearRangeStart + 20 > currentYear}
              aria-label="Later years"
            >
              <ChevronLeft size={16} style={{ transform: 'scaleX(-1)' }} />
            </button>
          </div>
          <div className="birthday-calendar-grid birthday-calendar-year-grid">
            {Array.from({ length: yearRangeEnd - yearRangeStart }, (_, i) => yearRangeStart + i).map((year) => {
              const isDisabled = year > currentYear;
              return (
                <button
                  key={year}
                  className={`birthday-calendar-selector${isDisabled ? ' birthday-calendar-selector--disabled' : ''}`}
                  onClick={() => handleYearSelect(year)}
                  disabled={isDisabled}
                >
                  {year}
                </button>
              );
            })}
          </div>
        </>
      )}

      {viewMode === 'month' && (
        <>
          <div className="birthday-calendar-header">
            <button className="birthday-calendar-back-btn" onClick={handleBackToYear} aria-label="Back to year">
              <ChevronLeft size={16} />
            </button>
            <div className="birthday-calendar-title">{selectedYear}</div>
            <div style={{ width: 28 }} />
          </div>
          <div className="birthday-calendar-grid birthday-calendar-month-grid">
            {['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'].map((month, idx) => {
              const isDisabled = selectedYear === currentYear && idx > today.getMonth();
              return (
                <button
                  key={month}
                  className={`birthday-calendar-selector${isDisabled ? ' birthday-calendar-selector--disabled' : ''}`}
                  onClick={() => handleMonthSelect(idx)}
                  disabled={isDisabled}
                >
                  {month.slice(0, 3)}
                </button>
              );
            })}
          </div>
        </>
      )}

      {viewMode === 'day' && (
        <>
          <div className="birthday-calendar-header">
            <button className="birthday-calendar-back-btn" onClick={handleBackToMonth} aria-label="Back to month">
              <ChevronLeft size={16} />
            </button>
            <div className="birthday-calendar-title">
              {monthName} {selectedYear}
            </div>
            <div style={{ width: 28 }} />
          </div>

          <div className="birthday-calendar-weekdays">
            {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'].map((day) => (
              <div key={day} className="birthday-calendar-weekday">
                {day}
              </div>
            ))}
          </div>

          <div className="birthday-calendar-grid birthday-calendar-day-grid">
            {monthDays.map((day, idx) => (
              <button
                key={idx}
                className={`birthday-calendar-day${day === null ? ' birthday-calendar-day--empty' : ''}${
                  day && isDateSelected(day) ? ' birthday-calendar-day--selected' : ''
                }${day && isDateDisabled(day) ? ' birthday-calendar-day--disabled' : ''}`}
                onClick={() => day && handleDaySelect(day)}
                disabled={day === null || (day ? isDateDisabled(day) : false)}
              >
                {day}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
