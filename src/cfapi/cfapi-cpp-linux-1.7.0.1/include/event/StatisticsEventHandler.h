#ifndef CFAPI_STATISTICSEVENTHANDLER_H
#define CFAPI_STATISTICSEVENTHANDLER_H

#include "StatisticsEvent.h"
#include "../data/MessageReader.h"

namespace cfapi
{

	class StatisticsEvent;

	/**
	* An object that registers to be notified when statistics generated
	*/

	class StatisticsEventHandler
	{
		public:
			virtual ~StatisticsEventHandler() {}
			/**
			* 
			* @param event
			*            carries the fired statistics event<br>
			*            Note this event is not preserved after return from this function.
			*/
			virtual void onStatisticsEvent(const StatisticsEvent& event) = 0;



	};

}

#endif
