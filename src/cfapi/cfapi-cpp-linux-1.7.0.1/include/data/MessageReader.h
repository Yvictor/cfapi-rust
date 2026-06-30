#ifndef CFAPI_MESSAGEREADER_H
#define CFAPI_MESSAGEREADER_H

#include "ValueTypes.h"
#include "DateTime.h"
#include <string>
#include "dllexport.h"

namespace cfapi_internal
{
	class MessageReaderImpl;
	class HostConfigImpl;
	class MessageEventImpl;
}

namespace cfapi
{


	/**
 	 * Created by an event that has content to process, the MessageReader is used to iterate over the token-value pairs in the payload. This
 	 * allows the user to decode all content in each token-value pair or skip to only the token-value pairs and content they desire.
 	 * 
 	 * Note: The following tokens will not be present in the MessageReader, but only in the MessageEvent:	<br>
	 *	3/PERMISSION 	use MessageEvent::getPermission()	<br>
	 *   	4/ENUM.SRC.ID 	use MessageEvent::getSource()	<br>
	 *   	5/SYMBOL.TICKER	use MessageEvent::getSymbol()	<br>
	 *	24/REFRESH 	use MessageEvent::getType()		<br>
	 *
	 * Also, if a query or subscribe was requested with an alternate index (such as CUSIP, SEDOL, ISIN, etc), the alternate index will not 
	 * be present in the MessageReader, but only in the MessageEvent via:	<br>
	 * 	MessageEvent::getAlternateIndexTokenNumber()			<br>
	 *      MessageEvent::getAlternateIndexTokenName()			<br>
	 *      MessageEvent::getAlternateIndexValue()			<br>
	 *
 	 * @see MessageEvent
 	 */
	class MessageReader
	{
		public:
			/**
			 * Indicates that there are no more token-value pairs in this message.
			 * Typically, the user would complete processing of this message/event and wait for the next event to arrive.  
			 */
    			static const int END_OF_MESSAGE = -1;

	    		/**
     			 * Returns next available CTF token ID and advances the internal pointer to this token-value pair. The user can call getValue*() if interested in processing this token-value pair,
	     		 * or call next() again to skip data block and get CTF token ID of next data block.
     			 * 
	     		 * @return CTF token ID number for this token-value pair; returns -1 when no more token-value pairs
	     		 */
				CFAPI_DLLEXPORTS int next();

			/**
			 * Returns the CTF token name of the current token-value pair
			 * 
			 * @return String containing the CTF token name, "UNKNOWN" if unknown CTF token name
			 * 
			 */
			CFAPI_DLLEXPORTS std::string getTokenName();

			/**
			 * Returns the CTF token nnumber of the current token-value pair
			 * 
			 * @return int containing the CTF token id; -1 if no current token-value pair
			 * 
			 */
			CFAPI_DLLEXPORTS int getTokenNumber();

			/**
			 * Returns the CTF token ID of the following data block, -1 if no data blocks follow the current block.
			 * Unlike next(), will not advance internal pointer to next block
			 * (position remains the same as it was when peek was called)
			 * 
			 * @return CTF token ID of following data block, -1 if no subsequent data blocks.
			 */
			CFAPI_DLLEXPORTS int peek();

			/**
			 * Searches the message for the specified token-value pair. 
			 * If found, MessageReader points to that token-value pair.
			 * If not found, position remains the same as it was when findDataBlock was called
			 * 
			 * @param blockId
			 *            CTF token ID to locate
			 * @return true if found, false if not
			 */
			CFAPI_DLLEXPORTS bool find(int blockId);



			/**
     			 * Returns the type of the value for this token-value pair
     			 * 
			 * @return ValueType of the value
			 * @throws APIException
			 */
			CFAPI_DLLEXPORTS ValueTypes getValueType();

			/**
     			 * If valueType==INT64, user can extract the value as an int64_t
     			 * 
			 * @return int64_t containing the current value
			 * @throws APIException
			 */
			CFAPI_DLLEXPORTS int64_t getValueAsInteger();

			/**
     			 * If valueType==DOUBLE or DATETIME, user can extract the value as a double
     			 * 
			 * @return double containing the current value
			 * @throws APIException
			 */
			CFAPI_DLLEXPORTS double getValueAsDouble();

			/**
     			 * If valueType==STRING, user can extract the value as a std::string
     			 * 
			 * @return std::string containing the current value
			 * @throws APIException
			 */
			CFAPI_DLLEXPORTS std::string getValueAsString();

			/**
     			 * If valueType==DATETIME, user can extract the value as a DateTime object
     			 * 
			 * @return DateTime object containing the current value
			 * Note: this DateTime object must be freed by the caller
			 * @throws APIException
			 */
			CFAPI_DLLEXPORTS DateTime getValueAsDateTime();

		private:
			friend class cfapi_internal::MessageEventImpl;
			MessageReader();
			~MessageReader();
			cfapi_internal::MessageReaderImpl *messageReaderImpl; 

	};

}
#endif
