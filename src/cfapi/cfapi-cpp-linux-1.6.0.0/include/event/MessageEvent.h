#ifndef CFAPI_RESPONSEEVENT_H
#define CFAPI_RESPONSEEVENT_H

#include "../data/MessageReader.h"

#include "dllexport.h"

namespace cfapi_internal
{
	class MessageEventImpl;
	class HostConfigImpl;
	class QueueReadHandler;
	class SessionImpl;
}

namespace cfapi
{ 
	class Session;

	/**
	 * MessageEvents represent messages received from the CSP.
	 * 
	 * @see MessageEventHandler
	 * @see Request
	 * 
	 */
	class MessageEvent
	{
		public:
			/**
			 * Type of MessageEvent
			 */
			enum Types
			{
        			/**
				 * Partial image received, rest will follow (e.g. snapshot message for one instrument in a multi-instrument snapshot request that is not the last)
				 */
				IMAGE_PART=0,

				/**
				 * Image is complete (e.g. the last (or only) message of a snapshot)
				 */
				IMAGE_COMPLETE=1,

				/**
				 * This event contains an update event.  (e.g. one message in a subscribe response)
				 */
				UPDATE=2,

				/**
				 * Refresh (e.g. refresh message in subscribe response)
				 */
				REFRESH=3,

				/**
				 * Event contains status information from the CSP
				 */
				STATUS=4

			};

			/**
			 * Returns the type of this MessageEvent
			 */
			CFAPI_DLLEXPORTS Types getType() const;

			/**
			 * Returns a message reader for processing the message content in this event
			 * 
			 * @return MessageReader that can be used to process this content
			 */
			CFAPI_DLLEXPORTS MessageReader & getReader() const;

			/**
			 * Returns the session associated with this event
			 * 
			 * @return Session that this event came from
			 */
			CFAPI_DLLEXPORTS Session& getSession() const;

			/**
			 * Is this message conflatable
			 * 
			 * @return 
			 *	-1 (unknown) if CONFLATION_INDICATOR_BOOL is false
			 *	0 (false) if not conflatable
			 *	1 (true) if conflatable
			 */
			CFAPI_DLLEXPORTS int isConflatable() const;

			/**
			 * Returns the symbol for this message
			 * 
			 * @return 
			 *	The symbol for this message
			 */
			CFAPI_DLLEXPORTS std::string getSymbol() const;

			/**
			 * Returns the permission for this message
			 * 
			 * @return 
			 *	The permission for this message
			 */
			CFAPI_DLLEXPORTS int getPermission() const;

			/**
			 * Returns the source for this message
			 * 
			 * @return 
			 *	The source for this message
			 */
			CFAPI_DLLEXPORTS int getSource() const;

			/**
			 * Returns the status code for this message when type == STATUS
			 * 
			 * @return 
			 *	The status code for this message
			 */
			CFAPI_DLLEXPORTS int getStatusCode() const;

			/**
			 * Returns the status string for the status code for this message when type == STATUS
			 * 
			 * @return 
			 *	The status string for this message
			 */
			CFAPI_DLLEXPORTS std::string getStatusString() const;

			/**
			 * Returns the API-generated tag for this message when present (type == STATUS | IMAGE_COMPLETE | IMAGE_PART), or 0 if not present.
			 * 
			 * @return 
			 *	The tag for this message, or 0 if not present.
			 */
			CFAPI_DLLEXPORTS int64_t getTag() const;

			/**
			 * Returns the CTF token number of the Alternate Index
			 * 
			 * @return 
			 *	The ctf token id, or -1 if not present.
			 */
			CFAPI_DLLEXPORTS int getAlternateIndexTokenNumber(size_t altidPos) const;

			/**
			 * Returns the CTF token name of the Alternate Index
			 * 
			 * @return 
			 *	The ctf token name, "UNKNOWN" if unknown, or empty string if Alternate Index is not present.
			 */
			CFAPI_DLLEXPORTS std::string getAlternateIndexTokenName(size_t altidPos) const;

			/**
			 * Returns the value of the Alternate Index
			 * 
			 * @return 
			 *	The value of the alternate index, or empty string if Alternate Index is not present.
			 */
			CFAPI_DLLEXPORTS std::string getAlternateIndexValue(size_t altidPos) const;

			/**
			 * Returns the number of Alternate Indices present in this message.
			 * Typically 0 or 1, but could be more if different subscriptions have used different alternate index types (such as CUSIP and ISIN) that map to the same symbol.
			 * 
			 * @return 
			 *	The number of alternate indices present in this message.
			 */

			CFAPI_DLLEXPORTS size_t getNumberofAlternateIndexes() const;

		private:
			MessageEvent();
			MessageEvent(cfapi_internal::SessionImpl *session, cfapi_internal::HostConfigImpl *hostConfig);
			~MessageEvent();	
			cfapi_internal::MessageEventImpl *messsgeEventImpl;
			friend class cfapi_internal::QueueReadHandler;
	};

}
#endif
