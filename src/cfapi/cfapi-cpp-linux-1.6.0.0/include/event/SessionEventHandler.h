#ifndef CFAPI_SESSIONEVENTHANDLER_H
#define CFAPI_SESSIONEVENTHANDLER_H

//#include "SessionEvent.h"


namespace cfapi
{

	class SessionEvent;

	/**
	* An object that registers to be notified of events generated by a Session object
	*/
	class SessionEventHandler
	{
		public:
			virtual ~SessionEventHandler() {}
	
			/**
			* 
			* @param sessionEvent
			*            carries the fired session event
			*/
			virtual void onSessionEvent(const SessionEvent& sessionEvent) = 0;
            
    };

}

#endif
