#ifndef CFAPI_SESSIONEVENT_H
#define CFAPI_SESSIONEVENT_H

#include "session/Session.h"

#include "dllexport.h"

namespace cfapi_internal
{
	class SessionEventImpl;
	class SessionImpl;
}


namespace cfapi
{
    class Session;

	/**
	 * SessionEvents represent information regarding the state of the connection(s) referenced by the session.
	 * Session Events are returned to the user via their SessionEventHandler implementation.
	 * 
	 * @see SessionEventHandler
	 * @see Session
	 * 
	 */
    class SessionEvent
    {
	public:
	
	/**
	 * enumeration of session event types
	 */
	enum Types
	{
		/**
		 *No connections established to any backend CSPs
		 */	
		CFAPI_SESSION_UNAVAILABLE=0,
		/**
		 *Connections established to all backend CSPs
		 */	
		CFAPI_SESSION_ESTABLISHED=1,
		/**
		 *Connection established to at least one CSP; trying to recover at least one CSP connection.
		 */	
		CFAPI_SESSION_RECOVERY=2,
		/**
		 *CDD downloaded from CSP; expect at session start, failover to a backup CSP, and during session when new CDD detected.
		 */	
		CFAPI_CDD_LOADED=3,
		/**
		*Connections established to all backend CSPs and send all sources available
		*/
		CFAPI_SESSION_AVAILABLE_ALLSOURCES = 4,
		/**
		*CSP health status; Connection to CSP(s) was restored and send the sources that became available
		*/
		CFAPI_SESSION_AVAILABLE_SOURCES = 5,
		/**
		*CSP health status; Connection to  one or more CSPs was lost and send the sources that are unavailable
		*/
		CFAPI_SESSION_RECOVERY_SOURCES=6,
		/**
		*Receive queue percentage full has exceeded the threshold
		*/
		CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD=7,
		/**
		*Receive queue percentage full has dropped below the threshold
		*/
		CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD=8,
		/**
		*One or more backend CSPs have started conflating all streaming data
		*/
		CFAPI_SESSION_JIT_START_CONFLATING=9,
		/**
		*All backend CSPs have stopped conflating 
		*/
		CFAPI_SESSION_JIT_STOP_CONFLATING=10,
		/**
		*New source has been added 
		*/
		CFAPI_SESSION_SOURCE_ADDED=11,
		/**
		*Source has been removed 
		*/
		CFAPI_SESSION_SOURCE_REMOVED=12,

	};

	/**
	 * Returns information about the specific type of event that occurred.
	 */
	CFAPI_DLLEXPORTS  Types getType() const;

	/**
	 * Returns the session that this event describes
	 */
	CFAPI_DLLEXPORTS  Session& getSession() const;

	/**
	 * Returns the current CDD version when event type is CFAPI_CDD_LOADED
	 */
	CFAPI_DLLEXPORTS  std::string getCddVersion() const;

	/**
	* Returns the source ID  when session event types becomes CFAPI_SESSION_AVAILABLE_ALLSOURCES / 
	* CFAPI_SESSION_AVAILABLE_SOURCES / CFAPI_SESSION_RECOVERY_SOURCES
	*/
	CFAPI_DLLEXPORTS  int getSourceID() const;

	/**
	* Returns the queue depth percent when session event type is CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD / 
	* CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD 
	*/
	CFAPI_DLLEXPORTS  int getQueueDepth() const;

	private:
		friend class cfapi_internal::SessionImpl;
		SessionEvent(Types type, Session &session, std::string cddVersion, int sourceID, int queueDepth);
		~SessionEvent();
		cfapi_internal::SessionEventImpl *sessionEventImpl;
    };

}

#endif
