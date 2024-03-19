#ifndef CFAPI_DATE_TIME_H_
#define CFAPI_DATE_TIME_H_

#include <stdint.h>
#include "dllexport.h"
namespace cfapi
{

	/**
 	 * Contains date specific values.  This information is obtained via the DateTime.date() method
 	 */
	class Date
	{

		public:

			/**
     			 * Returns the month of the year, where January == value 1.
     			 * 
	     		 * @return numeric value representing month of the year
	     		 */
			CFAPI_DLLEXPORTS int8_t month();

			/**
     			 * Returns the day of the month
     			 * 
	     		 * @return numeric value representing day of the month
	     		 */
			CFAPI_DLLEXPORTS int8_t day();

			/**
     			 * Returns the year
     			 * 
	     		 * @return numeric value representing the year, as a four digit year (e.g. 2016)
	     		 */
			CFAPI_DLLEXPORTS int16_t year();

		private:
			/**
     			 * Set the date
	     		 */
			CFAPI_DLLEXPORTS void setdate(int mon, int day, int year);

			int8_t m_month;
			int8_t m_day;
			int16_t m_year;
			friend class DateTime;
	};

	/**
 	 * Contains time specific values.  This information is obtained via the DateTime.time() method
 	 */
	class Time
	{

		public:

			/**
     			 * Returns the hour of the day
     			 * 
	     		 * @return numeric value representing hour of the day
	     		 */
			CFAPI_DLLEXPORTS int8_t hour();

			/**
     			 * Returns the minute of the hour
     			 * 
	     		 * @return numeric value representing the minute of the hour
	     		 */
			CFAPI_DLLEXPORTS int8_t minute();

			/**
     			 * Returns the second of the minute
     			 * 
	     		 * @return numeric value representing seconds of the minute
	     		 */
			CFAPI_DLLEXPORTS int8_t second();

			/**
     			 * Returns the millisecond of the second
     			 * 
	     		 * @return numeric value representing milliseconds of the second
	     		 */
			CFAPI_DLLEXPORTS int16_t millisecond();

			/**
     			 * Returns the microsecond of the millisecond
     			 * 
	     		 * @return numeric value representing microsecond of the millisecond
	     		 */
			CFAPI_DLLEXPORTS int16_t microsecond();


		private:
			/**
     			 * Set the time
	     		 */
			CFAPI_DLLEXPORTS void settime(int hour, int min, int sec, int16_t millisec, int16_t microsec, int16_t nanosec);

			int8_t m_hour;
			int8_t m_minute;
			int8_t m_second;
			int16_t m_millisecond;
			int16_t m_microsecond;
			int16_t m_nanosecond;
			friend class DateTime;
	};


	/**
 	 * Communicates Date and/or Time information.
 	 */
	class DateTime
	{
    
		public: 
			/**
     			 * Construct an empty DateTime
	     		 */
			CFAPI_DLLEXPORTS DateTime();

			/**
     			 * Construct DateTime using a double that represents seconds since 1/1/1970 UTC
	     		 */
			CFAPI_DLLEXPORTS DateTime(double utctime);

			/**
     			 * Indicates presence of the Date values
     			 * 
	     		 * @return true if Date is present, false if Date is not present
	     		 */
			CFAPI_DLLEXPORTS bool hasDate();
	
			/**
     			 * Returns DateTime.Date values when Date information is present (hasDate() returns true)
     			 * 
	     		 * @return DateTime.Date object
	     		 */
			CFAPI_DLLEXPORTS Date& date();

			/**
     			 * Indicates presence of the Time values
     			 * 
	     		 * @return true if Time is present, false if Time is not present
	     		 */
			CFAPI_DLLEXPORTS bool hasTime();

			/**
     			 * Returns DateTime.Time values when Time information is present (hasTime() returns true)
     			 * 
	     		 * @return DateTime.Time object
	     		 */
			CFAPI_DLLEXPORTS Time& time();

		private:
			bool m_hasDate;
			bool m_hasTime;
			Date m_date;
			Time m_time;

	};
};

#endif  /* CFAPI_DATE_TIME_H_ */
